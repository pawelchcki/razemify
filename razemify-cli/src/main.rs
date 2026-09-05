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
        return named_palette(name).map(Some).ok_or_else(|| {
            format!(
                "Unknown palette '{}'. Available: {}",
                name,
                all_palette_names().join(", ")
            )
            .into()
        });
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
    let path = match find_model_path(model_arg, model_type) {
        Some(path) => path,
        None => {
            eprintln!("Warning: No model found for {:?}. Will use existing alpha channel or opaque fallback.", model_type);
            eprintln!("  Set RAZEMIFY_MODEL_PATH or use --model/--model-type flags.");
            return None;
        }
    };

    let detected_type = ModelType::from_path(&path).unwrap_or(model_type);
    eprintln!("Loading {:?} model from: {}", detected_type, path.display());
    match RembgModel::load(&path, detected_type) {
        Ok(model) => {
            eprintln!("Model loaded successfully");
            Some(model)
        }
        Err(e) => {
            eprintln!(
                "Warning: Failed to load model: {}. Will use existing alpha or opaque fallback.",
                e
            );
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

fn is_image_up_to_date(source: &Path, target: &Path) -> bool {
    let check = || -> std::io::Result<bool> {
        let src_time = source.metadata()?.modified()?;
        let dst_time = target.metadata()?.modified()?;
        Ok(dst_time > src_time)
    };
    target.exists() && check().unwrap_or(false)
}

type PresetWork = Vec<(PathBuf, String, AlgorithmParams)>;

fn build_image_work(
    images: &[PathBuf],
    output_dir: &Path,
    presets: &[(String, AlgorithmParams)],
    force: bool,
) -> (Vec<(PathBuf, PresetWork)>, usize) {
    let mut image_work = Vec::new();
    let mut pre_skipped = 0usize;

    for image_path in images {
        let Some(stem) = image_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let pending: PresetWork = presets
            .iter()
            .filter_map(|(preset_name, params)| {
                let output_path = output_dir.join(format!("{}_{}.png", stem, preset_name));
                if !force && is_image_up_to_date(image_path, &output_path) {
                    return None;
                }
                Some((output_path, preset_name.clone(), params.clone()))
            })
            .collect();

        pre_skipped += presets.len() - pending.len();
        if !pending.is_empty() {
            image_work.push((image_path.clone(), pending));
        }
    }

    image_work.sort_by_key(|work| std::cmp::Reverse(work.1.len()));
    (image_work, pre_skipped)
}

fn apply_preset_to_image(
    image: &image::DynamicImage,
    alpha: &[u8],
    output_path: &Path,
    preset_name: &str,
    params: &AlgorithmParams,
    image_path: &Path,
) -> Result<(), String> {
    eprintln!("  Applying [{}] -> {}", preset_name, output_path.display());
    params
        .process(image, alpha)
        .map_err(|e| format!("{} [{}]: {}", image_path.display(), preset_name, e))?
        .save(output_path)
        .map_err(|e| {
            format!(
                "{} [{}]: save failed: {}",
                image_path.display(),
                preset_name,
                e
            )
        })?;
    eprintln!("  Done: {}", output_path.display());
    Ok(())
}

fn process_image_presets(
    image_path: &Path,
    pending_presets: &[(PathBuf, String, AlgorithmParams)],
    model: Option<&RembgModel>,
    all_errors: &mut Vec<String>,
) -> usize {
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
            return 0;
        }
    };

    let alpha = match extract_alpha(&img, model) {
        Ok(a) => Arc::new(a),
        Err(e) => {
            let msg = format!("{}: background removal failed: {}", image_path.display(), e);
            eprintln!("Error: {}", msg);
            all_errors.push(msg);
            return 0;
        }
    };

    let img_ref = &img;
    let errors: Vec<String> = pending_presets
        .par_iter()
        .filter_map(|(out_path, preset_name, params)| {
            apply_preset_to_image(img_ref, &alpha, out_path, preset_name, params, image_path).err()
        })
        .collect();

    let processed = pending_presets.len() - errors.len();
    all_errors.extend(errors);
    processed
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

    std::fs::create_dir_all(output_dir)?;
    let model = load_model_or_warn(model_path, model_type);

    if let Some(n) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .ok();
    }

    let (image_work, total_skipped) = build_image_work(&images, output_dir, &presets, force);
    let total_pending: usize = image_work.iter().map(|(_, p)| p.len()).sum();
    eprintln!(
        "To process: {} images ({} outputs), skipping {} up-to-date",
        image_work.len(),
        total_pending,
        total_skipped
    );

    let mut total_processed = 0usize;
    let mut all_errors = Vec::new();

    for (image_path, pending_presets) in &image_work {
        total_processed +=
            process_image_presets(image_path, pending_presets, model.as_ref(), &mut all_errors);
    }

    eprintln!(
        "\nDone! Processed: {}, Skipped: {}, Errors: {}",
        total_processed,
        total_skipped,
        all_errors.len()
    );
    for e in &all_errors {
        eprintln!("  {}", e);
    }

    Ok(())
}

fn compare_pixels(
    pa: &[u8; 3],
    pb: &[u8; 3],
    sum_abs: &mut [u64; 3],
    max_err: &mut [u32; 3],
) -> ([u8; 3], bool) {
    let mut diff_rgb = [0u8; 3];
    let mut exact = true;
    for c in 0..3 {
        let diff = (pa[c] as i32 - pb[c] as i32).unsigned_abs();
        if diff > 0 {
            exact = false;
        }
        sum_abs[c] += diff as u64;
        max_err[c] = max_err[c].max(diff);
        diff_rgb[c] = (diff * 4).min(255) as u8;
    }
    (diff_rgb, exact)
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

    let mut diff_img = diff_output.map(|_| RgbImage::new(width, height));

    for (x, y, pa) in rgb_a.enumerate_pixels() {
        let pb = rgb_b.get_pixel(x, y);
        let (diff_rgb, exact) = compare_pixels(&pa.0, &pb.0, &mut sum_abs_error, &mut max_error);
        if exact {
            exact_matches += 1;
        }
        if let Some(ref mut img) = diff_img {
            img.put_pixel(x, y, image::Rgb(diff_rgb));
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

fn resolve_single_params(
    preset: Option<&str>,
    thresh_low: u8,
    thresh_high: u8,
    clip_limit: f64,
    tile_size: u32,
    palette: Option<ColorPalette>,
) -> Result<(String, AlgorithmParams), Box<dyn std::error::Error>> {
    let (name, params) = match preset {
        Some(name) => {
            let p = AlgorithmParams::from_preset(name)
                .ok_or_else(|| format!("Unknown preset: {}", name))?;
            (name.to_string(), p)
        }
        None => (
            "detailed_strong".to_string(),
            AlgorithmParams::Detailed(DetailedParams {
                thresh_low,
                thresh_high,
                clip_limit,
                tile_size,
                palette: PALETTE_ORIGINAL,
            }),
        ),
    };

    let params = match palette {
        Some(pal) => params.with_palette(pal),
        None => params,
    };

    Ok((name, params))
}

fn build_batch_presets(
    preset: Option<&str>,
    all_palettes: bool,
    palette_name: Option<&str>,
    custom_palette: Option<ColorPalette>,
) -> Result<Vec<(String, AlgorithmParams)>, Box<dyn std::error::Error>> {
    let base_presets: Vec<(String, AlgorithmParams)> = match preset {
        Some(name) => {
            let p = AlgorithmParams::from_preset(name)
                .ok_or_else(|| format!("Unknown preset: {}", name))?;
            vec![(name.to_string(), p)]
        }
        None => AlgorithmParams::all_presets()
            .into_iter()
            .map(|(n, p)| (n.to_string(), p))
            .collect(),
    };

    if all_palettes {
        let mut result = Vec::with_capacity(base_presets.len() * all_palette_names().len());
        for (base_name, base_params) in &base_presets {
            for &pname in all_palette_names() {
                let pal = named_palette(pname).unwrap();
                result.push((
                    format!("{}_{}", base_name, pname),
                    base_params.clone().with_palette(pal),
                ));
            }
        }
        return Ok(result);
    }

    if let Some(pal) = custom_palette {
        let palette_label = palette_name.unwrap_or("custom");
        return Ok(base_presets
            .into_iter()
            .map(|(name, p)| (format!("{}_{}", name, palette_label), p.with_palette(pal)))
            .collect());
    }

    Ok(base_presets)
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
            let (preset_name, params) = resolve_single_params(
                preset.as_deref(),
                thresh_low,
                thresh_high,
                clip_limit,
                tile_size,
                resolved_palette,
            )?;
            cmd_single(
                &input,
                output.as_deref(),
                params,
                &preset_name,
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
            let presets = build_batch_presets(
                preset.as_deref(),
                all_palettes,
                palette.as_deref(),
                resolved_palette,
            )?;

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
