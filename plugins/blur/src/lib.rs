use std::{ffi::CStr, os::raw::c_char};
use serde::{Deserialize, Serialize};
use std::panic::{self, AssertUnwindSafe};

#[unsafe(no_mangle)]
/// # Safety
/// - `rgba_data` не должен быть null и должен быть выровнен для u8.
/// - Буфер по адресу `rgba_data` должен быть доступен для чтения и записи
///   в объеме не менее `width * height * 4` байт.
/// - Переданные `width` и `height` должны строго соответствовать реальным
///   физическим размерам изображения в буфере.
/// - `params` не должен быть null и должен указывать на память с нуль-терминатором (\0)
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

        blur_image(width as usize, height as usize, rgba_data, config);

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
    radius: u32,
    #[serde(default)]
    iterations: u32
}

fn parse_config(c_buf: *const c_char) -> Result<ImageConfig, String> {
    if c_buf.is_null() {
        return Ok(ImageConfig::default());
    }

    // Safety: Указатель c_buf валиден
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

fn blur_image(width: usize, height: usize, rgba_data: *mut u8, config: ImageConfig) {
    if rgba_data.is_null() || width == 0 || height == 0 || config.radius == 0 || config.iterations == 0 {
        return;
    }

    let total_pixels = width * height;

    // Safety: Указатель rgba_data валиден, указывает на буфер размером не меннее total_pixels * 4
    let pixel_slice = unsafe { std::slice::from_raw_parts_mut(rgba_data as *mut [u8; 4], total_pixels) };
    let mut working_buffer = pixel_slice.to_vec();

    let r = config.radius as isize;

    for iter in 0..config.iterations {
        let (read_h, write_h) = if iter % 2 == 0 {
            (pixel_slice as &[ [u8; 4] ], &mut working_buffer[..])
        } else {
            (&working_buffer[..], pixel_slice as &mut [ [u8; 4] ])
        };

        for y in 0..height {
            let row_offset = y * width;
            for x in 0..width {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut sum_a = 0u32;
                let mut count = 0u32;

                for k in -r..=r {
                    let nx = x as isize + k;
                    if nx >= 0 && nx < width as isize {
                        let pixel = read_h[row_offset + nx as usize];
                        sum_r += pixel[0] as u32;
                        sum_g += pixel[1] as u32;
                        sum_b += pixel[2] as u32;
                        sum_a += pixel[3] as u32;
                        count += 1;
                    }
                }

                write_h[row_offset + x] = [
                    (sum_r / count) as u8,
                    (sum_g / count) as u8,
                    (sum_b / count) as u8,
                    (sum_a / count) as u8,
                ];
            }
        }

        let (read_v, write_v) = if iter % 2 == 0 {
            (&working_buffer[..], pixel_slice as &mut [ [u8; 4] ])
        } else {
            (pixel_slice as &[ [u8; 4] ], &mut working_buffer[..])
        };

        for x in 0..width {
            for y in 0..height {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut sum_a = 0u32;
                let mut count = 0u32;

                for k in -r..=r {
                    let ny = y as isize + k;
                    if ny >= 0 && ny < height as isize {
                        let pixel = read_v[ny as usize * width + x];
                        sum_r += pixel[0] as u32;
                        sum_g += pixel[1] as u32;
                        sum_b += pixel[2] as u32;
                        sum_a += pixel[3] as u32;
                        count += 1;
                    }
                }

                write_v[y * width + x] = [
                    (sum_r / count) as u8,
                    (sum_g / count) as u8,
                    (sum_b / count) as u8,
                    (sum_a / count) as u8,
                ];
            }
        }
    }

    if config.iterations % 2 == 0 {
        // Safety: Указатели валидны и указывают на непересекающиеся области памяти размером не менее total_pixels * 4
        unsafe { std::ptr::copy_nonoverlapping(working_buffer.as_ptr() as *const u8, rgba_data, total_pixels * 4) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_parse_valid_json() {
        let json = CString::new(r#"{"radius": 5, "iterations": 4}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.radius, 5);
        assert_eq!(config.iterations, 4);
    }

    #[test]
    fn test_parse_missing_keys_uses_defaults() {
        let json = CString::new(r#"{}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.radius, 0);
        assert_eq!(config.iterations, 0);
    }

    #[test]
    fn test_parse_empty_string() {
        let json = CString::new("").unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.radius, 0);
        assert_eq!(config.iterations, 0);
    }

    #[test]
    fn test_parse_unknown_keys_ignored() {
        let json = CString::new(r#"{"radius": 5, "unknown_key": "hello", "fps": 60}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.radius, 5);
        assert_eq!(config.iterations, 0);
    }

    #[test]
    fn test_parse_invalid_types_returns_error() {
        let json = CString::new(r#"{"radius": "true", "iterations": true}"#).unwrap();
        let result = parse_config(json.as_ptr());
        
        assert!(result.is_err());
    }

    #[test]
    fn test_blur_monotone_buffer() {
        let mut buffer = vec![128u8; 3 * 3 * 4];
        let expected_backup = buffer.clone();

        let config = ImageConfig{ radius: 1, iterations: 1 };
        blur_image(3, 3, buffer.as_mut_ptr(), config);

        assert_eq!(buffer, expected_backup);
    }

    #[test]
    fn test_blur_center_pixel() {
        let mut buffer = vec![
            0, 0, 0, 255,   0, 0, 0, 255,   0, 0, 0, 255,
            0, 0, 0, 255,   255, 255, 255, 255,   0, 0, 0, 255,
            0, 0, 0, 255,   0, 0, 0, 255,   0, 0, 0, 255,
        ];

        let config = ImageConfig{ radius: 1, iterations: 1 };
        blur_image(3, 3, buffer.as_mut_ptr(), config);

        assert!(buffer[0 * 4] > 0, "Верхний левый пиксель остался нулевым после размытия");
        assert!(buffer[1 * 4] > 0, "Верхний центральный пиксель остался нулевым после размытия");
        assert!(buffer[3 * 4] > 0, "Средний левый пиксель остался нулевым после размытия");
        assert!(buffer[5 * 4] > 0, "Средний правый пиксель остался нулевым после размытия");

        assert!(buffer[4 * 4] < 255, "Центральный пиксель не отдал яркость соседям");
    }
}
