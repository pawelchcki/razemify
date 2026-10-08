use crate::exif_orientation::apply_exif_orientation_from_bytes;
use crate::pipeline::{extract_alpha_from_image, AlgorithmParams};
use crate::posterize::named_palette;
use image::GenericImageView;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// FFI error codes (must stay in sync with Dart side)
#[allow(dead_code)]
const ERROR_SUCCESS: i32 = 0; // Documented for clarity, though not explicitly returned
const ERROR_INVALID_PRESET_ENCODING: i32 = -1;
const ERROR_INVALID_PALETTE_ENCODING: i32 = -2;
const ERROR_IMAGE_DECODE_FAILED: i32 = -3;
const ERROR_UNKNOWN_PRESET: i32 = -4;
const ERROR_UNKNOWN_PALETTE: i32 = -5;
const ERROR_PROCESSING_FAILED: i32 = -6;

/// Process image from memory buffer (JPEG/PNG bytes) to raw RGB output.
/// Returns 0 on success, negative error codes on failure.
///
/// Output format: RGB bytes (width * height * 3), width, height written to out params.
///
/// # Safety
/// - input_data must be valid for input_len bytes
/// - output_data must have capacity for at least width*height*3 bytes
/// - preset_name and palette_name must be valid null-terminated UTF-8 strings
#[no_mangle]
pub unsafe extern "C" fn razemify_process_bytes(
    input_data: *const u8,
    input_len: usize,
    output_data: *mut u8,
    output_width: *mut u32,
    output_height: *mut u32,
    preset_name: *const c_char,
    palette_name: *const c_char,
) -> i32 {
    // Parse preset name
    let Ok(preset_str) = (unsafe { CStr::from_ptr(preset_name) }).to_str() else {
        return ERROR_INVALID_PRESET_ENCODING;
    };

    // Parse palette name
    let Ok(palette_str) = (unsafe { CStr::from_ptr(palette_name) }).to_str() else {
        return ERROR_INVALID_PALETTE_ENCODING;
    };

    // Load image from bytes
    let input_slice = unsafe { std::slice::from_raw_parts(input_data, input_len) };
    let Ok(img) = image::load_from_memory(input_slice) else {
        return ERROR_IMAGE_DECODE_FAILED;
    };

    // Apply EXIF orientation correction
    let img = apply_exif_orientation_from_bytes(img, input_slice);
    let (width, height) = img.dimensions();

    // Extract alpha channel
    let alpha = extract_alpha_from_image(&img);

    // Get preset parameters
    let Some(params) = AlgorithmParams::from_preset(preset_str) else {
        return ERROR_UNKNOWN_PRESET;
    };

    // Apply palette
    let Some(palette) = named_palette(palette_str) else {
        return ERROR_UNKNOWN_PALETTE;
    };
    let params = params.with_palette(palette);

    // Process image
    let Ok(result) = params.process(&img, &alpha) else {
        return ERROR_PROCESSING_FAILED;
    };

    // Copy RGB data to output buffer
    let rgb_bytes = result.into_raw();
    let output_slice = unsafe { std::slice::from_raw_parts_mut(output_data, rgb_bytes.len()) };
    output_slice.copy_from_slice(&rgb_bytes);

    // Write dimensions
    unsafe {
        *output_width = width;
        *output_height = height;
    }

    0 // Success
}

/// Get required output buffer size for an image.
/// Returns 0 on success, negative on failure.
///
/// # Safety
/// - input_data must be valid for input_len bytes
#[no_mangle]
pub unsafe extern "C" fn razemify_get_output_size(
    input_data: *const u8,
    input_len: usize,
    width_out: *mut u32,
    height_out: *mut u32,
) -> i32 {
    let input_slice = unsafe { std::slice::from_raw_parts(input_data, input_len) };
    let img = match image::load_from_memory(input_slice) {
        Ok(img) => img,
        Err(_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "razemify_get_output_size failed to decode image: {:?}",
                _error
            );
            return ERROR_IMAGE_DECODE_FAILED; // Failed to decode image (consistent with razemify_process_bytes)
        }
    };

    // Apply EXIF orientation correction to get correct dimensions
    let img = apply_exif_orientation_from_bytes(img, input_slice);

    let (width, height) = img.dimensions();
    unsafe {
        *width_out = width;
        *height_out = height;
    }

    0 // Success
}

fn str_slice_to_c_char(slice: &[&str]) -> *const c_char {
    let json = serde_json::to_string(slice).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).map_or(std::ptr::null(), |s| s.into_raw() as *const c_char)
}

/// Get JSON array of available presets.
/// Returns a null-terminated string that must be freed with razemify_free_string.
#[no_mangle]
pub extern "C" fn razemify_list_presets() -> *const c_char {
    let presets: Vec<&str> = AlgorithmParams::all_presets()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    str_slice_to_c_char(&presets)
}

/// Get JSON array of available palettes.
/// Returns a null-terminated string that must be freed with razemify_free_string.
#[no_mangle]
pub extern "C" fn razemify_list_palettes() -> *const c_char {
    let palettes = crate::posterize::all_palette_names();
    str_slice_to_c_char(palettes)
}

/// Free string returned by list functions.
///
/// # Safety
/// - s must be a pointer returned by razemify_list_presets or razemify_list_palettes
#[no_mangle]
pub unsafe extern "C" fn razemify_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}
