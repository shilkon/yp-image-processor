use std::{ffi::CStr, os::raw::c_char};
use serde::{Deserialize, Serialize};
use std::panic::{self, AssertUnwindSafe};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn process_image(
    width: u32,
    height: u32,
    rgba_data: *mut u8,
    params: *const c_char,
) -> i32 {
    if rgba_data.is_null() {
        return -1;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(move || {
        let config = match parse_config(params) {
            Ok(image_config) => image_config,
            Err(e) => {
                eprintln!("Ошибка парсинга параметров: {e}");
                return Err(-2);
            }
        };

        mirror_image(width as usize, height as usize, rgba_data, config);

        Ok(0)
    }));

    match result {
        Ok(inner_result) => match inner_result {
            Ok(success_code) => success_code,
            Err(error_code) => error_code,
        },
        Err(_panic_payload) => {
            eprintln!("Паника внутри плагина");
            -999
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ImageConfig {
    #[serde(default)]
    horizontal: bool,
    #[serde(default)]
    vertical: bool
}

fn parse_config(c_buf: *const c_char) -> Result<ImageConfig, String> {
    if c_buf.is_null() {
        return Ok(ImageConfig::default());
    }

    let c_str = unsafe { CStr::from_ptr(c_buf) };
    
    let trimmed = std::str::from_utf8(c_str.to_bytes())
        .map_err(|e| format!("Невалидный UTF-8: {}", e))?
        .trim();

    if trimmed.is_empty() {
        return Ok(ImageConfig::default());
    }

    serde_json::from_str::<ImageConfig>(trimmed)
        .map_err(|e| format!("Ошибка JSON: {}", e))
}

fn mirror_image(width: usize, height: usize, rgba_data: *mut u8, config: ImageConfig) {
    if config.horizontal {
        let total_pixels = width * height;
        let pixel_slice = unsafe {std::slice::from_raw_parts_mut(rgba_data as *mut [u8; 4], total_pixels) };

        let w = width as usize;

        for row in pixel_slice.chunks_exact_mut(w) {
            let mut left = 0;
            let mut right = w - 1;
            
            while left < right {
                row.swap(left, right);
                left += 1;
                right -= 1;
            }
        }
    }

    if config.vertical {
        let row_stride = width * 4;
        let total_bytes = row_stride * height;

        let byte_slice = unsafe { std::slice::from_raw_parts_mut(rgba_data, total_bytes) };

        let mut rows: Vec<&mut [u8]> = byte_slice.chunks_exact_mut(row_stride).collect();

        let mut top = 0;
        let mut bottom = height - 1;

        while top < bottom {
            let top_row_ptr = rows[top].as_mut_ptr();
            let bottom_row_ptr = rows[bottom].as_mut_ptr();

            unsafe { std::ptr::swap_nonoverlapping(top_row_ptr, bottom_row_ptr, row_stride) };

            top += 1;
            bottom -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_parse_valid_json() {
        let json = CString::new(r#"{"horizontal": true, "vertical": false}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.horizontal, true);
        assert_eq!(config.vertical, false);
    }

    #[test]
    fn test_parse_missing_keys_uses_defaults() {
        let json = CString::new(r#"{}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.horizontal, false);
        assert_eq!(config.vertical, false);
    }

    #[test]
    fn test_parse_empty_string() {
        let json = CString::new("").unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.horizontal, false);
        assert_eq!(config.vertical, false);
    }

    #[test]
    fn test_parse_unknown_keys_ignored() {
        let json = CString::new(r#"{"horizontal": true, "unknown_key": "hello", "fps": 60}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.horizontal, true);
        assert_eq!(config.vertical, false);
    }

    #[test]
    fn test_parse_invalid_types_returns_error() {
        let json = CString::new(r#"{"horizontal": "true", "vertical": true}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_err());
    }

    fn create_test_buffer_2x2() -> Vec<u8> {
        vec![
            1, 1, 1, 255,   2, 2, 2, 255,
            3, 3, 3, 255,   4, 4, 4, 255
        ]
    }

    #[test]
    fn test_flip_horizontal_2x2() {
        let mut buffer = create_test_buffer_2x2();
        let config = ImageConfig{ horizontal: true, vertical: false };
        
        mirror_image(2, 2, buffer.as_mut_ptr(), config);

        let expected = vec![
            2, 2, 2, 255,   1, 1, 1, 255,
            4, 4, 4, 255,   3, 3, 3, 255
        ];

        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_flip_vertical_2x2() {
        let mut buffer = create_test_buffer_2x2();
        let config = ImageConfig{ horizontal: false, vertical: true };
        
        mirror_image(2, 2, buffer.as_mut_ptr(), config);

        let expected = vec![
            3, 3, 3, 255,   4, 4, 4, 255,
            1, 1, 1, 255,   2, 2, 2, 255
        ];

        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_flip_both_horizontal_and_vertical_2x2() {
        let mut buffer = create_test_buffer_2x2();
        let config = ImageConfig{ horizontal: true, vertical: true };
        
        mirror_image(2, 2, buffer.as_mut_ptr(), config);

        let expected = vec![
            4, 4, 4, 255,   3, 3, 3, 255,
            2, 2, 2, 255,   1, 1, 1, 255
        ];

        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_flip_nothing_2x2() {
        let mut buffer = create_test_buffer_2x2();
        let config = ImageConfig{ horizontal: false, vertical: false };
        
        mirror_image(2, 2, buffer.as_mut_ptr(), config);

        let expected = vec![
            1, 1, 1, 255,   2, 2, 2, 255,
            3, 3, 3, 255,   4, 4, 4, 255
        ];

        assert_eq!(buffer, expected);
    }
}
