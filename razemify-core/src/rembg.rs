/// Background removal using ONNX models via ort.
///
/// Supports multiple model architectures:
/// - U2Net (320x320, simple /255 normalization)
/// - BiRefNet (1024x1024, ImageNet normalization, sigmoid + min-max output)
/// - ISNet (1024x1024, ImageNet normalization, sigmoid output)
use image::{DynamicImage, GenericImageView, GrayImage, RgbaImage};
use ndarray::Array4;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::Mutex;

/// Supported background removal model architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    U2Net,
    BiRefNet,
    ISNet,
}

impl ModelType {
    /// Input resolution expected by the model.
    pub fn input_size(self) -> u32 {
        match self {
            ModelType::U2Net => 320,
            ModelType::BiRefNet | ModelType::ISNet => 1024,
        }
    }

    /// Parse model type from string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "u2net" => Some(ModelType::U2Net),
            "birefnet" => Some(ModelType::BiRefNet),
            "isnet" => Some(ModelType::ISNet),
            _ => None,
        }
    }

    /// Try to detect model type from filename.
    pub fn from_path(path: &Path) -> Option<Self> {
        let stem = path.file_stem()?.to_str()?.to_lowercase();
        if stem.contains("birefnet") {
            Some(ModelType::BiRefNet)
        } else if stem.contains("isnet") {
            Some(ModelType::ISNet)
        } else if stem.contains("u2net") {
            Some(ModelType::U2Net)
        } else {
            None
        }
    }

    pub fn all_names() -> &'static [&'static str] {
        &["u2net", "birefnet", "isnet"]
    }
}

// ImageNet normalization constants
const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

pub struct RembgModel {
    session: Mutex<Session>,
    model_type: ModelType,
}

impl RembgModel {
    /// Load ONNX model from the given path with specified model type.
    pub fn load(
        model_path: &Path,
        model_type: ModelType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = Session::builder()?.with_intra_threads(4)?;

        // BiRefNet models trigger a shape inference bug in ONNX Runtime 1.23.x
        // (github.com/microsoft/onnxruntime/issues/26261). Using Level1 (basic)
        // optimization works around this by transforming nodes before shape inference.
        if model_type == ModelType::BiRefNet {
            builder = builder.with_optimization_level(GraphOptimizationLevel::Level1)?;
        }

        let session = builder.commit_from_file(model_path)?;

        Ok(Self {
            session: Mutex::new(session),
            model_type,
        })
    }

    /// Remove background from image, returning RGBA image with alpha mask.
    pub fn remove_background(
        &self,
        img: &DynamicImage,
    ) -> Result<RgbaImage, Box<dyn std::error::Error>> {
        let (orig_w, orig_h) = img.dimensions();
        let rgb = img.to_rgb8();
        let input_size = self.model_type.input_size();

        // Resize to model input size
        let resized = image::imageops::resize(
            &rgb,
            input_size,
            input_size,
            image::imageops::FilterType::Lanczos3,
        );

        let input = prepare_input_tensor(&resized, self.model_type, input_size as usize);
        let input_tensor = Tensor::from_array(input)?;

        // Run inference
        let mut session = self
            .session
            .lock()
            .map_err(|e| format!("Failed to lock session: {}", e))?;
        let outputs = session.run(ort::inputs![input_tensor])?;
        let output_value = &outputs[0];

        // Extract as ndarray view
        let output_array = output_value.try_extract_array::<f32>()?;

        // Extract mask - output shape is typically (1, 1, H, W)
        let shape = output_array.shape();
        let mask_h = shape[2];
        let mask_w = shape[3];

        let mask = match self.model_type {
            ModelType::U2Net | ModelType::ISNet => {
                extract_standard_mask(&output_array, mask_w, mask_h)
            }
            ModelType::BiRefNet => extract_birefnet_mask(&output_array, mask_w, mask_h),
        };

        // Resize mask back to original dimensions
        let mask_resized =
            image::imageops::resize(&mask, orig_w, orig_h, image::imageops::FilterType::Lanczos3);

        Ok(apply_alpha_mask(&rgb, &mask_resized))
    }
}

fn normalize_u2net(resized: &image::RgbImage, input_size: usize) -> Array4<f32> {
    let mut input = Array4::<f32>::zeros((1, 3, input_size, input_size));
    for (x, y, pixel) in resized.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        input[[0, 0, y, x]] = pixel[0] as f32 / 255.0;
        input[[0, 1, y, x]] = pixel[1] as f32 / 255.0;
        input[[0, 2, y, x]] = pixel[2] as f32 / 255.0;
    }
    input
}

fn normalize_imagenet(resized: &image::RgbImage, input_size: usize) -> Array4<f32> {
    let mut input = Array4::<f32>::zeros((1, 3, input_size, input_size));
    for (x, y, pixel) in resized.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        for c in 0..3 {
            let val = pixel[c] as f32 / 255.0;
            input[[0, c, y, x]] = (val - IMAGENET_MEAN[c]) / IMAGENET_STD[c];
        }
    }
    input
}

fn prepare_input_tensor(
    resized: &image::RgbImage,
    model_type: ModelType,
    input_size: usize,
) -> Array4<f32> {
    match model_type {
        ModelType::U2Net => normalize_u2net(resized, input_size),
        ModelType::BiRefNet | ModelType::ISNet => normalize_imagenet(resized, input_size),
    }
}

fn extract_standard_mask(output: &ndarray::ArrayViewD<f32>, w: usize, h: usize) -> GrayImage {
    let mut mask = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let sig = sigmoid(output[[0, 0, y, x]]);
            mask.put_pixel(x as u32, y as u32, image::Luma([(sig * 255.0) as u8]));
        }
    }
    mask
}

fn extract_birefnet_mask(output: &ndarray::ArrayViewD<f32>, w: usize, h: usize) -> GrayImage {
    let mut sig_values = Vec::with_capacity(w * h);
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;

    for y in 0..h {
        for x in 0..w {
            let sig = sigmoid(output[[0, 0, y, x]]);
            sig_values.push(sig);
            min_val = min_val.min(sig);
            max_val = max_val.max(sig);
        }
    }

    let diff = max_val - min_val;
    let range = if diff < 1e-6 { 1.0 } else { diff };

    let mut mask = GrayImage::new(w as u32, h as u32);
    for (i, &sig) in sig_values.iter().enumerate() {
        let x = (i % w) as u32;
        let y = (i / w) as u32;
        let normalized = (sig - min_val) / range;
        mask.put_pixel(x, y, image::Luma([(normalized * 255.0) as u8]));
    }
    mask
}

fn apply_alpha_mask(rgb: &image::RgbImage, mask: &GrayImage) -> RgbaImage {
    let (w, h) = rgb.dimensions();
    let mut rgba = RgbaImage::new(w, h);
    for (x, y, pixel) in rgb.enumerate_pixels() {
        let alpha = mask.get_pixel(x, y)[0];
        let alpha_binary = if alpha > 128 { 255 } else { 0 };
        rgba.put_pixel(
            x,
            y,
            image::Rgba([pixel[0], pixel[1], pixel[2], alpha_binary]),
        );
    }
    rgba
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Helper function to check for model files in a directory
fn check_directory_for_models(
    dir: &std::path::Path,
    filenames: &[&str],
) -> Option<std::path::PathBuf> {
    filenames
        .iter()
        .map(|filename| dir.join(filename))
        .find(|path| path.exists())
}

fn candidate_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs_path() {
        dirs.push(home.join(".razemify").join("models"));
        dirs.push(home.join(".u2net"));
    }
    dirs.push(std::path::PathBuf::from("."));
    dirs
}

/// Try to find a model path, searching for the preferred model type first.
pub fn find_model_path(
    explicit_path: Option<&Path>,
    model_type: ModelType,
) -> Option<std::path::PathBuf> {
    // 1. Explicit path
    if let Some(p) = explicit_path {
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // 2. Environment variable
    if let Ok(env_path) = std::env::var("RAZEMIFY_MODEL_PATH") {
        let p = std::path::PathBuf::from(&env_path);
        if p.exists() {
            return Some(p);
        }
    }

    let search_dirs = candidate_search_dirs();

    // 3-5. Preferred model type in candidate directories
    let filenames = model_filenames(model_type);
    for dir in &search_dirs {
        if let Some(path) = check_directory_for_models(dir, &filenames) {
            return Some(path);
        }
    }

    // 6-8. Fallback: try any known model file in candidate directories
    let all_filenames = [
        "BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx",
        "BiRefNet-general-epoch_244.onnx",
        "u2net.onnx",
        "isnet-general-use.onnx",
    ];

    for dir in &search_dirs {
        if let Some(path) = check_directory_for_models(dir, &all_filenames) {
            return Some(path);
        }
    }

    None
}

/// Return expected filenames for a given model type.
fn model_filenames(model_type: ModelType) -> Vec<&'static str> {
    match model_type {
        ModelType::BiRefNet => vec![
            "BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx",
            "BiRefNet-general-epoch_244.onnx",
        ],
        ModelType::U2Net => vec!["u2net.onnx"],
        ModelType::ISNet => vec!["isnet-general-use.onnx"],
    }
}

fn dirs_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .ok()
            .map(std::path::PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(std::path::PathBuf::from)
    }
}

/// Extract alpha channel from an existing RGBA image.
/// Used as fallback when no model is available.
pub fn extract_existing_alpha(img: &DynamicImage) -> Option<Vec<u8>> {
    if let DynamicImage::ImageRgba8(rgba) = img {
        Some(rgba.pixels().map(|p| p[3]).collect())
    } else {
        None
    }
}
