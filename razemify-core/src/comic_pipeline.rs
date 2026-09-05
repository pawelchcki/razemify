/// Comic-style posterization pipeline:
/// Grayscale → CLAHE → Posterize (3 colors) + Sobel edges on CLAHE gray → Overlay edges → Output
///
/// Key difference from "detailed": no sharpening step (edges provide definition instead),
/// plus bold outlines composited on top using the palette's bg color.
use image::{DynamicImage, GenericImageView, RgbImage};

use crate::clahe::clahe;
use crate::edge_detect::{overlay_edges, sobel_magnitude, threshold_and_dilate};
use crate::pipeline::rgb_to_grayscale;
use crate::posterize::{posterize, ColorPalette, PALETTE_ORIGINAL};

/// Processing parameters for the comic algorithm.
#[derive(Debug, Clone)]
pub struct ComicParams {
    pub thresh_low: u8,
    pub thresh_high: u8,
    pub clip_limit: f64,
    pub tile_size: u32,
    pub edge_threshold: u8,
    pub edge_width: u8,
    pub palette: ColorPalette,
}

impl ComicParams {
    const fn new(
        thresh_low: u8,
        thresh_high: u8,
        clip_limit: f64,
        tile_size: u32,
        edge_threshold: u8,
        edge_width: u8,
    ) -> Self {
        Self {
            thresh_low,
            thresh_high,
            clip_limit,
            tile_size,
            edge_threshold,
            edge_width,
            palette: PALETTE_ORIGINAL,
        }
    }

    /// Classic comic look: thick outlines, moderate contrast.
    pub fn comic_bold() -> Self {
        Self::new(80, 160, 3.0, 8, 40, 3)
    }

    /// Pen-and-ink style: thin detailed lines, lower threshold catches finer detail.
    pub fn comic_fine() -> Self {
        Self::new(80, 160, 3.0, 8, 25, 1)
    }

    /// Gritty high-contrast: stronger CLAHE + medium edges.
    pub fn comic_heavy() -> Self {
        Self::new(70, 150, 4.5, 8, 50, 2)
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        match name {
            "comic_bold" => Some(Self::comic_bold()),
            "comic_fine" => Some(Self::comic_fine()),
            "comic_heavy" => Some(Self::comic_heavy()),
            _ => None,
        }
    }

    pub fn all_presets() -> Vec<(&'static str, Self)> {
        vec![
            ("comic_bold", Self::comic_bold()),
            ("comic_fine", Self::comic_fine()),
            ("comic_heavy", Self::comic_heavy()),
        ]
    }

    /// Return a copy with a different palette applied.
    pub fn with_palette(mut self, palette: ColorPalette) -> Self {
        self.palette = palette;
        self
    }
}

/// Process an image through the comic pipeline with a pre-computed alpha channel.
pub fn process_image_comic_with_alpha(
    img: &DynamicImage,
    alpha: &[u8],
    params: &ComicParams,
) -> Result<RgbImage, Box<dyn std::error::Error>> {
    let (width, height) = img.dimensions();

    // Step 1: Convert to grayscale using BT.601
    let gray = rgb_to_grayscale(img);

    // Step 2: Apply CLAHE
    let enhanced = clahe(&gray, width, height, params.clip_limit, params.tile_size);

    // Step 3: Posterize to 3 colors (no sharpening — edges provide definition)
    let mut rgb_data = posterize(
        &enhanced,
        alpha,
        width,
        height,
        params.thresh_low,
        params.thresh_high,
        &params.palette,
    );

    // Step 4: Sobel edge detection on the CLAHE-enhanced grayscale
    let magnitudes = sobel_magnitude(&enhanced, width, height);
    let edges = threshold_and_dilate(
        &magnitudes,
        params.edge_threshold,
        params.edge_width,
        width,
        height,
    );

    // Step 5: Overlay edges using the palette's bg color
    overlay_edges(&mut rgb_data, &edges, 1.0, params.palette.bg);

    // Create output image
    RgbImage::from_raw(width, height, rgb_data)
        .ok_or_else(|| "Failed to create output image".into())
}
