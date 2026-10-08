/// C ABI exports for shared library consumers.
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;

use crate::pipeline::{process_file, DetailedParams};
use crate::posterize::PALETTE_ORIGINAL;
use crate::rembg::{ModelType, RembgModel};

/// Opaque handle to a loaded U2Net model.
pub struct RazemifyModel {
    inner: RembgModel,
}

/// Load U2Net model from path. Returns null on failure.
///
/// # Safety
/// `model_path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn razemify_load_model(model_path: *const c_char) -> *mut RazemifyModel {
    if model_path.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(path_str) = CStr::from_ptr(model_path).to_str() else {
        return std::ptr::null_mut();
    };

    let path = Path::new(path_str);
    let model_type = ModelType::from_path(path).unwrap_or(ModelType::U2Net);
    match RembgModel::load(path, model_type) {
        Ok(model) => Box::into_raw(Box::new(RazemifyModel { inner: model })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Free a loaded model.
///
/// # Safety
/// `model` must be a valid pointer returned by `razemify_load_model`, or null.
#[no_mangle]
pub unsafe extern "C" fn razemify_free_model(model: *mut RazemifyModel) {
    if !model.is_null() {
        drop(Box::from_raw(model));
    }
}

/// Process a single image file with the detailed algorithm.
/// Returns 0 on success, non-zero on failure.
///
/// # Safety
/// - `input_path` and `output_path` must be valid null-terminated UTF-8 strings.
/// - `model` may be null (will use existing alpha or fully opaque fallback).
#[no_mangle]
pub unsafe extern "C" fn razemify_process(
    input_path: *const c_char,
    output_path: *const c_char,
    model: *const RazemifyModel,
    thresh_low: u8,
    thresh_high: u8,
    clip_limit: f64,
    tile_size: u32,
) -> i32 {
    if input_path.is_null() || output_path.is_null() {
        return -1;
    }
    let Ok(input) = CStr::from_ptr(input_path).to_str() else {
        return -1;
    };
    let Ok(output) = CStr::from_ptr(output_path).to_str() else {
        return -2;
    };

    let model_ref = (!model.is_null()).then(|| &(*model).inner);

    let params = DetailedParams {
        thresh_low,
        thresh_high,
        clip_limit,
        tile_size,
        palette: PALETTE_ORIGINAL,
    };

    if process_file(Path::new(input), Path::new(output), model_ref, &params).is_ok() {
        0
    } else {
        -3
    }
}
