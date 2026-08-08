use clap::{Parser, Subcommand};
use image::{GenericImageView, RgbImage};
use rayon::prelude::*;
use razemify_core::exif_orientation::apply_exif_orientation;
use razemify_core::pipeline::{extract_alpha, AlgorithmParams, DetailedParams};
use razemify_core::posterize::{all_palette_names, named_palette, ColorPalette, PALETTE_ORIGINAL};
use razemify_core::rembg::{find_model_path, ModelType, RembgModel};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "razemify-cli",
    about = "Posterize images with the razemify detailed algorithm"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process a single image
    Single {
        /// Input image path
        input: PathBuf,

        /// Output image path (default: input_detailed_strong.png)
        output: Option<PathBuf>,

        /// Low threshold for shadow/midtone boundary
        #[arg(long, default_value_t = 70)]
        thresh_low: u8,

        /// High threshold for midtone/highlight boundary
        #[arg(long, default_value_t = 150)]
        thresh_high: u8,

        /// CLAHE clip limit
        #[arg(long, default_value_t = 4.0)]
        clip_limit: f64,

        /// CLAHE tile grid size
        #[arg(long, default_value_t = 8)]
        tile_size: u32,

        /// Use a named preset (overrides individual params)
        #[arg(long)]
        preset: Option<String>,

        /// Color palette name: original, burgundy, burgundy_teal, burgundy_gold, rose, cmyk
        #[arg(long)]
        palette: Option<String>,

        /// Custom colors as 3 hex values: BG,MIDTONE,HIGHLIGHT (e.g. "#000000,#720546,#FFFFFF")
        #[arg(long)]
        colors: Option<String>,

        /// Path to ONNX model file
        #[arg(long)]
        model: Option<PathBuf>,

        /// Model type: u2net, birefnet, isnet (default: auto-detect from filename, or birefnet)
        #[arg(long)]
        model_type: Option<String>,
    },

    /// Process all images in a directory
    Batch {
        /// Input directory
        input_dir: PathBuf,

        /// Output directory (default: input_dir/output_rs)
        output_dir: Option<PathBuf>,

        /// Run only a specific preset (default: all 3 detailed presets)
        #[arg(long)]
        preset: Option<String>,

        /// Color palette name: original, burgundy, burgundy_teal, burgundy_gold, rose, cmyk
        #[arg(long)]
        palette: Option<String>,

        /// Custom colors as 3 hex values: BG,MIDTONE,HIGHLIGHT (e.g. "#000000,#720546,#FFFFFF")
        #[arg(long)]
        colors: Option<String>,

        /// Run all palette variants (one output per palette per preset per image)
        #[arg(long)]
        all_palettes: bool,

        /// Number of parallel jobs (default: num_cpus)
        #[arg(long, short)]
        jobs: Option<usize>,

        /// Reprocess even if output is up-to-date
        #[arg(long)]
        force: bool,

        /// Path to ONNX model file
        #[arg(long)]
        model: Option<PathBuf>,

        /// Model type: u2net, birefnet, isnet (default: auto-detect from filename, or birefnet)
        #[arg(long)]
        model_type: Option<String>,
    },

    /// Compare two images pixel-by-pixel
    Compare {
        /// First image
        image_a: PathBuf,

        /// Second image
        image_b: PathBuf,

        /// Save visual diff to this path
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp"];
const PRESET_SUFFIXES: &[&str] = &[
    "detailed_standard",
    "detailed_strong",
    "detailed_fine",
    "comic_bold",
    "comic_fine",
    "comic_heavy",
    "dark",
    "balanced",
    "bright",
    "contrast",
    "soft",
    "painted_smooth",
    "painted_detail",
    "painted_abstract",
];

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_generated_file(path: &Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    PRESET_SUFFIXES.iter().any(|suffix| stem.ends_with(suffix))
        || all_palette_names()
            .iter()
            .any(|pname| stem.ends_with(pname))
}

fn default_output_path(input: &Path, preset_name: &str) -> PathBuf {
    let stem = input.file_stem().unwrap().to_str().unwrap();
    let parent = input.parent().unwrap_or(Path::new("."));
    parent.join(format!("{}_{}.png", stem, preset_name))
}

fn resolve_palette(
    palette_name: &Option<String>,
    colors_str: &Option<String>,
) -> Result<Option<ColorPalette>, Box<dyn std::error::Error>> {
    if let Some(ref name) = palette_name {
        match named_palette(name) {
            Some(p) => return Ok(Some(p)),
            None => {
                return Err(format!(
                    "Unknown palette '{}'. Available: {}",
                    name,
                    all_palette_names().join(", ")
                )
                .into())
            }
        }
    }

    if let Some(ref colors) = colors_str {
        let parts: Vec<&str> = colors.split(',').collect();
        if parts.len() != 3 {
            return Err("--colors requires exactly 3 hex values separated by commas (e.g. '#000000,#720546,#FFFFFF')".into());
        }
        let bg = ColorPalette::parse_hex(parts[0].trim())?;
        let mid = ColorPalette::parse_hex(parts[1].trim())?;
        let hi = ColorPalette::parse_hex(parts[2].trim())?;
        return Ok(Some(ColorPalette::new(bg, mid, hi)));
    }

    Ok(None)
}

fn resolve_model_type(model_type_arg: &Option<String>, model_path: Option<&Path>) -> ModelType {
    // 1. Explicit --model-type flag
    if let Some(ref name) = model_type_arg {
        if let Some(mt) = ModelType::from_name(name) {
            return mt;
        }
        eprintln!(
            "Warning: Unknown model type '{}'. Available: {}. Falling back to auto-detect.",
            name,
            ModelType::all_names().join(", ")
        );
    }

    // 2. Auto-detect from model filename
    if let Some(path) = model_path {
        if let Some(mt) = ModelType::from_path(path) {
            return mt;
        }
    }

    // 3. Default to BiRefNet
    ModelType::BiRefNet
}

fn load_model_or_warn(model_arg: Option<&Path>, model_type: ModelType) -> Option<RembgModel> {
    let model_path = find_model_path(model_arg, model_type);
    match model_path {
        Some(path) => {
            let detected_type = ModelType::from_path(&path).unwrap_or(model_type);
            eprintln!("Loading {:?} model from: {}", detected_type, path.display());
            match RembgModel::load(&path, detected_type) {
                Ok(model) => {
                    eprintln!("Model loaded successfully");
                    Some(model)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load model: {}. Will use existing alpha or opaque fallback.", e);
                    None
                }
            }
        }
        None => {
            eprintln!("Warning: No model found for {:?}. Will use existing alpha channel or opaque fallback.", model_type);
            eprintln!("  Set RAZEMIFY_MODEL_PATH or use --model/--model-type flags.");
            None
        }
    }
}

fn cmd_single(
    input: &Path,
    output: Option<&Path>,
    params: AlgorithmParams,
    preset_name: &str,
    model_path: Option<&Path>,
    model_type: ModelType,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(input, preset_name));

    let model = load_model_or_warn(model_path, model_type);

    eprintln!(
        "Processing: {} -> {}",
        input.display(),
        output_path.display()
    );
    eprintln!("Preset: {}", preset_name);

    let img = image::open(input)?;
    let img = apply_exif_orientation(img, input);
    let alpha = extract_alpha(&img, model.as_ref())?;
    let result = params.process(&img, &alpha)?;
    result.save(&output_path)?;
    eprintln!("Done: {}", output_path.display());
    Ok(())
}

fn cmd_batch(
    input_dir: &Path,
    output_dir: &Path,
    presets: Vec<(String, AlgorithmParams)>,
    jobs: Option<usize>,
    force: bool,
    model_path: Option<&Path>,
    model_type: ModelType,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find all source images
    let images: Vec<PathBuf> = std::fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image_file(p) && !is_generated_file(p))
        .collect();

    if images.is_empty() {
        eprintln!("No source images found in {}", input_dir.display());
        return Ok(());
    }

    eprintln!(
        "Found {} source images, {} presets",
        images.len(),
        presets.len()
    );

    // Create output directory
    std::fs::create_dir_all(output_dir)?;

    // Load model once
    let model = load_model_or_warn(model_path, model_type);

    // Configure thread pool
    if let Some(n) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .ok();
    }

    let presets = Arc::new(presets);
    let mut total_processed = 0usize;
    let mut all_errors: Vec<String> = Vec::new();

    // Pre-compute pending work per image and sort: most pending first
    type PresetWork = Vec<(PathBuf, String, AlgorithmParams)>;
    let mut image_work: Vec<(PathBuf, PresetWork)> = Vec::new();
    let mut pre_skipped = 0usize;

    for image_path in &images {
        let stem = image_path.file_stem().unwrap().to_str().unwrap();

        let pending: Vec<(PathBuf, String, AlgorithmParams)> = presets
            .iter()
            .filter_map(|(preset_name, params)| {
                let output_path = output_dir.join(format!("{}_{}.png", stem, preset_name));

                if !force && output_path.exists() {
                    if let (Ok(in_meta), Ok(out_meta)) =
                        (image_path.metadata(), output_path.metadata())
                    {
                        if let (Ok(in_time), Ok(out_time)) =
                            (in_meta.modified(), out_meta.modified())
                        {
                            if out_time > in_time {
                                return None;
                            }
                        }
                    }
                }

                Some((output_path, preset_name.clone(), params.clone()))
            })
            .collect();

        pre_skipped += presets.len() - pending.len();

        if !pending.is_empty() {
            image_work.push((image_path.clone(), pending));
        }
    }

    // Sort: images with most pending presets first (new images before partially done ones)
    image_work.sort_by_key(|work| std::cmp::Reverse(work.1.len()));

    let total_skipped = pre_skipped;

    let total_pending: usize = image_work.iter().map(|(_, p)| p.len()).sum();
    eprintln!(
        "To process: {} images ({} outputs), skipping {} up-to-date",
        image_work.len(),
        total_pending,
        total_skipped
    );

    // Process each image: remove background once, then apply all presets
    for (image_path, pending_presets) in &image_work {
        // Load image and remove background once
        eprintln!(
            "\nLoading & removing background: {} ({} presets to apply)",
            image_path.display(),
            pending_presets.len()
        );

        let img = match image::open(image_path) {
            Ok(img) => apply_exif_orientation(img, image_path),
            Err(e) => {
                let msg = format!("{}: failed to load: {}", image_path.display(), e);
                eprintln!("Error: {}", msg);
                all_errors.push(msg);
                continue;
            }
        };

        let alpha = match extract_alpha(&img, model.as_ref()) {
            Ok(a) => Arc::new(a),
            Err(e) => {
                let msg = format!("{}: background removal failed: {}", image_path.display(), e);
                eprintln!("Error: {}", msg);
                all_errors.push(msg);
                continue;
            }
        };

        // Apply all presets in parallel, reusing the same image + alpha
        let img_ref = &img;
        let errors: Vec<_> = pending_presets
            .par_iter()
            .filter_map(|(output_path, preset_name, params)| {
                eprintln!("  Applying [{}] -> {}", preset_name, output_path.display());
                match params.process(img_ref, &alpha) {
                    Ok(result) => match result.save(output_path) {
                        Ok(()) => {
                            eprintln!("  Done: {}", output_path.display());
                            None
                        }
                        Err(e) => {
                            let msg = format!(
                                "{} [{}]: save failed: {}",
                                image_path.display(),
                                preset_name,
                                e
                            );
                            eprintln!("  Error: {}", msg);
                            Some(msg)
                        }
                    },
                    Err(e) => {
                        let msg = format!("{} [{}]: {}", image_path.display(), preset_name, e);
                        eprintln!("  Error: {}", msg);
                        Some(msg)
                    }
                }
            })
            .collect();

        total_processed += pending_presets.len() - errors.len();
        all_errors.extend(errors);
    }

    eprintln!(
        "\nDone! Processed: {}, Skipped: {}, Errors: {}",
        total_processed,
        total_skipped,
        all_errors.len()
    );
    if !all_errors.is_empty() {
        for e in &all_errors {
            eprintln!("  {}", e);
        }
    }

    Ok(())
}

fn cmd_compare(
    image_a: &Path,
    image_b: &Path,
    diff_output: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let a = image::open(image_a)?;
    let b = image::open(image_b)?;

    let (wa, ha) = a.dimensions();
    let (wb, hb) = b.dimensions();

    if wa != wb || ha != hb {
        println!(
            "Images have different dimensions: {}x{} vs {}x{}",
            wa, ha, wb, hb
        );
        return Ok(());
    }

    let rgb_a = a.to_rgb8();
    let rgb_b = b.to_rgb8();

    let width = wa;
    let height = ha;
    let total_pixels = (width * height) as u64;

    let mut exact_matches = 0u64;
    let mut sum_abs_error = [0u64; 3];
    let mut max_error = [0u32; 3];

    let mut diff_img = if diff_output.is_some() {
        Some(RgbImage::new(width, height))
    } else {
        None
    };

    for y in 0..height {
        for x in 0..width {
            let pa = rgb_a.get_pixel(x, y);
            let pb = rgb_b.get_pixel(x, y);

            let mut pixel_match = true;
            for c in 0..3 {
                let diff = (pa[c] as i32 - pb[c] as i32).unsigned_abs();
                if diff > 0 {
                    pixel_match = false;
                }
                sum_abs_error[c] += diff as u64;
                max_error[c] = max_error[c].max(diff);

                if let Some(ref mut img) = diff_img {
                    let vis = (diff * 4).min(255) as u8;
                    img.get_pixel_mut(x, y)[c] = vis;
                }
            }

            if pixel_match {
                exact_matches += 1;
            }
        }
    }

    let match_pct = (exact_matches as f64 / total_pixels as f64) * 100.0;
    let mae: Vec<f64> = sum_abs_error
        .iter()
        .map(|&s| s as f64 / total_pixels as f64)
        .collect();

    println!(
        "Image comparison: {} vs {}",
        image_a.display(),
        image_b.display()
    );
    println!("Dimensions: {}x{}", width, height);
    println!("Total pixels: {}", total_pixels);
    println!("Exact matches: {} ({:.2}%)", exact_matches, match_pct);
    println!(
        "MAE per channel (R,G,B): {:.4}, {:.4}, {:.4}",
        mae[0], mae[1], mae[2]
    );
    println!(
        "Max error per channel (R,G,B): {}, {}, {}",
        max_error[0], max_error[1], max_error[2]
    );

    if let (Some(img), Some(out_path)) = (diff_img, diff_output) {
        img.save(out_path)?;
        println!("Visual diff saved to: {}", out_path.display());
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Single {
            input,
            output,
            thresh_low,
            thresh_high,
            clip_limit,
            tile_size,
            preset,
            palette,
            colors,
            model,
            model_type,
        } => {
            let mt = resolve_model_type(&model_type, model.as_deref());
            let resolved_palette = resolve_palette(&palette, &colors)?;

            let (preset_name, params) = if let Some(ref name) = preset {
                let p = AlgorithmParams::from_preset(name)
                    .ok_or_else(|| format!("Unknown preset: {}", name))?;
                (name.as_str(), p)
            } else {
                (
                    "detailed_strong",
                    AlgorithmParams::Detailed(DetailedParams {
                        thresh_low,
                        thresh_high,
                        clip_limit,
                        tile_size,
                        palette: PALETTE_ORIGINAL,
                    }),
                )
            };

            let params = if let Some(pal) = resolved_palette {
                params.with_palette(pal)
            } else {
                params
            };

            let preset_name_owned = preset_name.to_string();
            cmd_single(
                &input,
                output.as_deref(),
                params,
                &preset_name_owned,
                model.as_deref(),
                mt,
            )?;
        }

        Commands::Batch {
            input_dir,
            output_dir,
            preset,
            palette,
            colors,
            all_palettes,
            jobs,
            force,
            model,
            model_type,
        } => {
            let output = output_dir.unwrap_or_else(|| input_dir.join("output_rs"));
            let mt = resolve_model_type(&model_type, model.as_deref());

            let resolved_palette = resolve_palette(&palette, &colors)?;

            // Build base presets (processing params without palette)
            let base_presets: Vec<(&str, AlgorithmParams)> = if let Some(ref name) = preset {
                let p = AlgorithmParams::from_preset(name)
                    .ok_or_else(|| format!("Unknown preset: {}", name))?;
                vec![(leak_str(name.clone()), p)]
            } else {
                AlgorithmParams::all_presets()
            };

            // Build final presets with palette variations
            let presets: Vec<(String, AlgorithmParams)> = if all_palettes {
                // Cross-product: each base preset x each palette
                let mut result = Vec::new();
                for (base_name, base_params) in &base_presets {
                    for &pname in all_palette_names() {
                        let pal = named_palette(pname).unwrap();
                        let name = format!("{}_{}", base_name, pname);
                        result.push((name, base_params.clone().with_palette(pal)));
                    }
                }
                result
            } else if let Some(pal) = resolved_palette {
                // Single palette override
                let palette_label = palette.as_deref().unwrap_or("custom");
                base_presets
                    .into_iter()
                    .map(|(name, p)| {
                        let full_name = format!("{}_{}", name, palette_label);
                        (full_name, p.with_palette(pal))
                    })
                    .collect()
            } else {
                // Default: original palette
                base_presets
                    .into_iter()
                    .map(|(name, p)| (name.to_string(), p))
                    .collect()
            };

            cmd_batch(
                &input_dir,
                &output,
                presets,
                jobs,
                force,
                model.as_deref(),
                mt,
            )?;
        }

        Commands::Compare {
            image_a,
            image_b,
            output,
        } => {
            cmd_compare(&image_a, &image_b, output.as_deref())?;
        }
    }

    Ok(())
}

/// Leak a String to get a &'static str. Used for preset names in batch mode.
fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
