/// Full processing pipeline:
/// load image -> rembg -> grayscale (BT.601) -> CLAHE -> sharpen -> posterize -> save PNG
use image::{DynamicImage, GenericImageView, RgbImage};

use crate::clahe::clahe;
use crate::comic_pipeline::{process_image_comic_with_alpha, ComicParams};
use crate::posterize::{posterize, ColorPalette, PALETTE_ORIGINAL};
use crate::sharpen::sharpen;

#[cfg(feature = "rembg")]
use crate::exif_orientation::apply_exif_orientation;
#[cfg(feature = "rembg")]
use crate::rembg::{extract_existing_alpha, RembgModel};
#[cfg(feature = "rembg")]
use std::path::Path;

/// Processing parameters for the detailed algorithm.
#[derive(Debug, Clone)]
pub struct DetailedParams {
    pub thresh_low: u8,
    pub thresh_high: u8,
    pub clip_limit: f64,
    pub tile_size: u32,
    pub palette: ColorPalette,
}

impl DetailedParams {
    const fn new(thresh_low: u8, thresh_high: u8, clip_limit: f64, tile_size: u32) -> Self {
        Self {
            thresh_low,
            thresh_high,
            clip_limit,
            tile_size,
            palette: PALETTE_ORIGINAL,
        }
    }

    pub fn detailed_standard() -> Self {
        Self::new(80, 160, 3.0, 8)
    }

    pub fn detailed_strong() -> Self {
        Self::new(70, 150, 4.0, 8)
    }

    pub fn detailed_fine() -> Self {
        Self::new(80, 160, 2.5, 4)
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        match name {
            "detailed_standard" => Some(Self::detailed_standard()),
            "detailed_strong" => Some(Self::detailed_strong()),
            "detailed_fine" => Some(Self::detailed_fine()),
            _ => None,
        }
    }

    pub fn all_presets() -> Vec<(&'static str, Self)> {
        vec![
            ("detailed_standard", Self::detailed_standard()),
            ("detailed_strong", Self::detailed_strong()),
            ("detailed_fine", Self::detailed_fine()),
        ]
    }

    /// Return a copy with a different palette applied.
    pub fn with_palette(mut self, palette: ColorPalette) -> Self {
        self.palette = palette;
        self
    }
}

/// Convert RGB to grayscale using BT.601 integer formula (matches OpenCV).
/// gray = (R*4899 + G*9617 + B*1868 + 8192) >> 14
pub fn rgb_to_grayscale(img: &DynamicImage) -> Vec<u8> {
    img.to_rgb8()
        .pixels()
        .map(|p| {
            let (r, g, b) = (p[0] as u32, p[1] as u32, p[2] as u32);
            ((r * 4899 + g * 9617 + b * 1868 + 8192) >> 14).min(255) as u8
        })
        .collect()
}

/// Extract alpha channel from an image using background removal model or fallback.
#[cfg(feature = "rembg")]
pub fn extract_alpha(
    img: &DynamicImage,
    model: Option<&RembgModel>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if let Some(model) = model {
        let rgba = model.remove_background(img)?;
        Ok(rgba.pixels().map(|p| p[3]).collect())
    } else if let Some(existing_alpha) = extract_existing_alpha(img) {
        Ok(existing_alpha)
    } else {
        let (width, height) = img.dimensions();
        Ok(vec![255u8; (width * height) as usize])
    }
}

/// Extract alpha channel from an image (without rembg support).
pub fn extract_alpha_from_image(img: &DynamicImage) -> Vec<u8> {
    if img.color().has_alpha() {
        img.to_rgba8().pixels().map(|p| p[3]).collect()
    } else {
        let (width, height) = img.dimensions();
        vec![255u8; (width * height) as usize]
    }
}

/// Process a single image through the full detailed pipeline.
#[cfg(feature = "rembg")]
pub fn process_image(
    img: &DynamicImage,
    model: Option<&RembgModel>,
    params: &DetailedParams,
) -> Result<RgbImage, Box<dyn std::error::Error>> {
    let alpha = extract_alpha(img, model)?;
    process_image_with_alpha(img, &alpha, params)
}

/// Process an image with a pre-computed alpha channel (skips background removal).
pub fn process_image_with_alpha(
    img: &DynamicImage,
    alpha: &[u8],
    params: &DetailedParams,
) -> Result<RgbImage, Box<dyn std::error::Error>> {
    let (width, height) = img.dimensions();

    // Step 1: Convert to grayscale using BT.601
    let gray = rgb_to_grayscale(img);

    // Step 2: Apply CLAHE
    let enhanced = clahe(&gray, width, height, params.clip_limit, params.tile_size);

    // Step 3: Apply sharpening
    let sharpened = sharpen(&enhanced, width, height);

    // Step 4: Posterize to 3 colors
    let rgb_data = posterize(
        &sharpened,
        alpha,
        width,
        height,
        params.thresh_low,
        params.thresh_high,
        &params.palette,
    );

    // Create output image
    RgbImage::from_raw(width, height, rgb_data)
        .ok_or_else(|| "Failed to create output image".into())
}

/// Process and save a single image file.
#[cfg(feature = "rembg")]
pub fn process_file(
    input_path: &Path,
    output_path: &Path,
    model: Option<&RembgModel>,
    params: &DetailedParams,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(input_path)?;
    let img = apply_exif_orientation(img, input_path);
    let result = process_image(&img, model, params)?;
    result.save(output_path)?;
    Ok(())
}

/// Unified algorithm parameters wrapping both detailed and comic pipelines.
#[derive(Debug, Clone)]
pub enum AlgorithmParams {
    Detailed(DetailedParams),
    Comic(ComicParams),
}

impl AlgorithmParams {
    /// Apply a palette to whichever variant this is.
    pub fn with_palette(self, palette: ColorPalette) -> Self {
        match self {
            AlgorithmParams::Detailed(p) => AlgorithmParams::Detailed(p.with_palette(palette)),
            AlgorithmParams::Comic(p) => AlgorithmParams::Comic(p.with_palette(palette)),
        }
    }

    /// Process an image with a pre-computed alpha channel.
    pub fn process(
        &self,
        img: &DynamicImage,
        alpha: &[u8],
    ) -> Result<RgbImage, Box<dyn std::error::Error>> {
        match self {
            AlgorithmParams::Detailed(p) => process_image_with_alpha(img, alpha, p),
            AlgorithmParams::Comic(p) => process_image_comic_with_alpha(img, alpha, p),
        }
    }

    /// Look up a preset by name across both algorithm families.
    pub fn from_preset(name: &str) -> Option<Self> {
        DetailedParams::from_preset(name)
            .map(AlgorithmParams::Detailed)
            .or_else(|| ComicParams::from_preset(name).map(AlgorithmParams::Comic))
    }

    /// All presets from both algorithm families.
    pub fn all_presets() -> Vec<(&'static str, Self)> {
        DetailedParams::all_presets()
            .into_iter()
            .map(|(name, p)| (name, AlgorithmParams::Detailed(p)))
            .chain(
                ComicParams::all_presets()
                    .into_iter()
                    .map(|(name, p)| (name, AlgorithmParams::Comic(p))),
            )
            .collect()
    }
}
