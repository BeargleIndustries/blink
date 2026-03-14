//! Safe wrapper around sd.cpp's ESRGAN upscaler context.

use std::ffi::{c_void, CString};
use crate::error::SdError;
use crate::types::GeneratedImage;

/// Thin owning wrapper around `*mut sd_sys::upscaler_ctx_t`.
/// Freed on drop via `sd_sys::free_upscaler_ctx`.
pub struct UpscalerContext {
    ctx: *mut sd_sys::upscaler_ctx_t,
}

// upscaler_ctx_t is single-threaded from our side; safe to Send.
unsafe impl Send for UpscalerContext {}

impl UpscalerContext {
    /// Load an ESRGAN model and create the upscaler context.
    pub fn new(esrgan_path: &str, n_threads: i32) -> Result<Self, SdError> {
        if !std::path::Path::new(esrgan_path).exists() {
            return Err(SdError::ModelNotFound { path: esrgan_path.to_owned() });
        }

        let path_c = CString::new(esrgan_path).map_err(|_| SdError::InvalidParams {
            reason: "esrgan_path contains interior NUL byte".into(),
        })?;

        let ctx = unsafe {
            sd_sys::new_upscaler_ctx(
                path_c.as_ptr(),
                false, // offload_params_to_cpu
                false, // direct
                n_threads,
                512,   // tile_size
            )
        };

        if ctx.is_null() {
            return Err(SdError::ContextCreationFailed {
                reason: "new_upscaler_ctx returned null — check model path and format".into(),
            });
        }

        Ok(Self { ctx })
    }

    /// Upscale an image (PNG/JPEG bytes) by the given integer factor.
    ///
    /// Returns a `GeneratedImage` with RGBA data, ready for PNG encoding.
    pub fn upscale(&self, image_data: &[u8], factor: u32) -> Result<GeneratedImage, SdError> {
        // Decode input image to RGB
        let img = image::load_from_memory(image_data).map_err(|e| SdError::InvalidParams {
            reason: format!("Failed to decode input image: {}", e),
        })?;
        let rgb_img = img.to_rgb8();
        let (w, h) = rgb_img.dimensions();
        let mut rgb_data: Vec<u8> = rgb_img.into_raw();

        let input_image = sd_sys::sd_image_t {
            width: w,
            height: h,
            channel: 3,
            data: rgb_data.as_mut_ptr(),
        };

        // upscale() returns sd_image_t BY VALUE (not a pointer)
        let result = unsafe {
            sd_sys::upscale(self.ctx, input_image, factor)
        };

        let out_w = result.width;
        let out_h = result.height;
        let out_ch = result.channel;
        let pixel_count = (out_w as usize) * (out_h as usize) * (out_ch as usize);

        if result.data.is_null() || pixel_count == 0 {
            // Free if non-null before returning error
            if !result.data.is_null() {
                unsafe { libc::free(result.data as *mut c_void); }
            }
            return Err(SdError::InferenceReturnedNull);
        }

        // Copy out pixel data
        let src_slice = unsafe { std::slice::from_raw_parts(result.data, pixel_count) };

        let rgba_data = if out_ch == 3 {
            let mut rgba = Vec::with_capacity((out_w as usize) * (out_h as usize) * 4);
            for pixel in src_slice.chunks_exact(3) {
                rgba.push(pixel[0]);
                rgba.push(pixel[1]);
                rgba.push(pixel[2]);
                rgba.push(255);
            }
            rgba
        } else {
            src_slice.to_vec()
        };

        // Free the data pointer allocated by sd.cpp
        unsafe { libc::free(result.data as *mut c_void); }

        Ok(GeneratedImage {
            data: rgba_data,
            width: out_w,
            height: out_h,
        })
    }
}

impl Drop for UpscalerContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            log::info!("Freeing upscaler context");
            unsafe {
                sd_sys::free_upscaler_ctx(self.ctx);
            }
        }
    }
}
