//! Safe FFI bridge over raw sd.cpp bindings.
//!
//! All `unsafe` interactions with sd-sys live here. The rest of sd-wrapper
//! treats `SdCppContext` as a safe, Send-able handle.

use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::SdError;
use crate::progress::{ProgressCallback, ProgressUpdate};
use crate::types::*;

/// Thin owning wrapper around `*mut sd_sys::sd_ctx_t`.
/// Freed on drop via `sd_sys::free_sd_ctx`.
pub(crate) struct SdCppContext {
    ctx: *mut sd_sys::sd_ctx_t,
}

// sd_ctx_t is internally synchronized by sd.cpp (single-threaded use from our
// dedicated inference thread), so Send is sound.
unsafe impl Send for SdCppContext {}

impl SdCppContext {
    /// Load a model and create the sd.cpp context.
    pub(crate) fn new(config: &ContextConfig) -> Result<Self, SdError> {
        // Validate model path exists (belt-and-suspenders; caller also checks)
        if !std::path::Path::new(&config.model_path).exists() {
            return Err(SdError::ModelNotFound {
                path: config.model_path.clone(),
            });
        }

        let model_path_c = CString::new(config.model_path.as_str()).map_err(|_| {
            SdError::InvalidParams {
                reason: "model_path contains interior NUL byte".into(),
            }
        })?;

        let vae_path_c = match &config.vae_path {
            Some(p) => Some(CString::new(p.as_str()).map_err(|_| SdError::InvalidParams {
                reason: "vae_path contains interior NUL byte".into(),
            })?),
            None => None,
        };

        unsafe {
            let mut params: sd_sys::sd_ctx_params_t = std::mem::zeroed();
            sd_sys::sd_ctx_params_init(&mut params);

            params.model_path = model_path_c.as_ptr();
            if let Some(ref vae) = vae_path_c {
                params.vae_path = vae.as_ptr();
            }
            params.n_threads = config.n_threads;
            params.vae_decode_only = true;
            // SD_TYPE_COUNT = auto-detect quantization from model file
            params.wtype = sd_sys::sd_type_t_SD_TYPE_COUNT;

            log::info!(
                "Creating sd.cpp context: model={}, threads={}",
                config.model_path,
                config.n_threads
            );

            let ctx = sd_sys::new_sd_ctx(&params);
            if ctx.is_null() {
                return Err(SdError::ContextCreationFailed {
                    reason: "new_sd_ctx returned null — model may be corrupted or incompatible"
                        .into(),
                });
            }

            Ok(Self { ctx })
        }
    }

    /// Run image generation (txt2img when `input_image` is `None`, img2img otherwise).
    pub(crate) fn generate(
        &self,
        params: &GenerationParams,
        input_image: Option<&[u8]>,
        strength: f32,
        progress_cb: Option<ProgressCallback>,
        cancel_flag: &AtomicBool,
    ) -> Result<GeneratedImage, SdError> {
        // Pre-flight cancel check
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(SdError::Cancelled);
        }

        // Validate inputs
        if params.width == 0 || params.height == 0 {
            return Err(SdError::InvalidParams {
                reason: "width and height must be > 0".into(),
            });
        }
        if params.width > 4096 || params.height > 4096 {
            return Err(SdError::InvalidParams {
                reason: "width and height must be <= 4096".into(),
            });
        }
        if params.steps == 0 {
            return Err(SdError::InvalidParams {
                reason: "steps must be > 0".into(),
            });
        }

        let prompt_c = CString::new(params.prompt.as_str()).map_err(|_| {
            SdError::InvalidParams {
                reason: "prompt contains interior NUL byte".into(),
            }
        })?;
        let neg_prompt_c =
            CString::new(params.negative_prompt.as_str()).map_err(|_| SdError::InvalidParams {
                reason: "negative_prompt contains interior NUL byte".into(),
            })?;

        // Decode input image to raw RGB if provided (for img2img)
        let mut decoded_rgb: Vec<u8> = Vec::new();
        let mut init_image = sd_sys::sd_image_t {
            width: 0,
            height: 0,
            channel: 0,
            data: std::ptr::null_mut(),
        };

        if let Some(img_bytes) = input_image {
            let img = image::load_from_memory(img_bytes).map_err(|e| SdError::InvalidParams {
                reason: format!("Failed to decode input image: {}", e),
            })?;
            let rgb_img = img.to_rgb8();
            let (w, h) = rgb_img.dimensions();
            decoded_rgb = rgb_img.into_raw();
            init_image = sd_sys::sd_image_t {
                width: w,
                height: h,
                channel: 3,
                data: decoded_rgb.as_mut_ptr(),
            };
        }

        // Set up progress trampoline
        let trampoline_data = ProgressTrampolineData {
            callback: progress_cb,
        };

        unsafe {
            // Install progress callback (GLOBAL in sd.cpp)
            sd_sys::sd_set_progress_callback(
                Some(progress_trampoline),
                &trampoline_data as *const ProgressTrampolineData as *mut c_void,
            );

            // Build generation params
            let mut gen_params: sd_sys::sd_img_gen_params_t = std::mem::zeroed();
            sd_sys::sd_img_gen_params_init(&mut gen_params);

            gen_params.prompt = prompt_c.as_ptr();
            gen_params.negative_prompt = neg_prompt_c.as_ptr();
            gen_params.width = params.width as i32;
            gen_params.height = params.height as i32;
            gen_params.seed = params.seed;
            gen_params.batch_count = 1;

            // Sample params
            let mut sample_params: sd_sys::sd_sample_params_t = std::mem::zeroed();
            sd_sys::sd_sample_params_init(&mut sample_params);
            sample_params.sample_method = params.sample_method.to_c();
            sample_params.sample_steps = params.steps as i32;
            sample_params.scheduler = sd_sys::scheduler_t_KARRAS_SCHEDULER;
            sample_params.guidance.txt_cfg = params.cfg_scale;
            gen_params.sample_params = sample_params;

            // img2img specifics
            if input_image.is_some() {
                gen_params.init_image = init_image;
                gen_params.strength = strength;
            } else {
                gen_params.strength = 0.0;
            }

            log::info!(
                "Calling generate_image: prompt='{}', {}x{}, {} steps, seed={}",
                params.prompt,
                params.width,
                params.height,
                params.steps,
                params.seed
            );

            let result_ptr = sd_sys::generate_image(self.ctx, &gen_params);

            // Clear progress callback immediately
            sd_sys::sd_set_progress_callback(None, std::ptr::null_mut());

            if result_ptr.is_null() {
                return Err(SdError::InferenceReturnedNull);
            }

            // Read the first image (batch_count = 1)
            let sd_img = &*result_ptr;
            let w = sd_img.width;
            let h = sd_img.height;
            let ch = sd_img.channel;
            let pixel_count = (w as usize) * (h as usize) * (ch as usize);

            if sd_img.data.is_null() || pixel_count == 0 {
                libc::free(result_ptr as *mut c_void);
                return Err(SdError::InferenceReturnedNull);
            }

            // Copy pixel data out before freeing
            let src_slice = std::slice::from_raw_parts(sd_img.data, pixel_count);

            let rgba_data = if ch == 3 {
                // Convert RGB -> RGBA
                let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
                for pixel in src_slice.chunks_exact(3) {
                    rgba.push(pixel[0]);
                    rgba.push(pixel[1]);
                    rgba.push(pixel[2]);
                    rgba.push(255);
                }
                rgba
            } else {
                // Already RGBA or single channel — copy as-is
                src_slice.to_vec()
            };

            // Free the sd_image_t data allocated by sd.cpp
            libc::free(sd_img.data as *mut c_void);
            libc::free(result_ptr as *mut c_void);

            Ok(GeneratedImage {
                data: rgba_data,
                width: w,
                height: h,
            })
        }
    }
}

impl Drop for SdCppContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            log::info!("Freeing sd.cpp context");
            unsafe {
                sd_sys::free_sd_ctx(self.ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Progress callback trampoline
// ---------------------------------------------------------------------------

struct ProgressTrampolineData {
    callback: Option<ProgressCallback>,
}

/// C-compatible callback forwarded from sd.cpp's global progress system.
unsafe extern "C" fn progress_trampoline(step: c_int, steps: c_int, time: f32, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let cb_data = &*(data as *const ProgressTrampolineData);
    if let Some(ref cb) = cb_data.callback {
        cb(ProgressUpdate {
            step: step as u32,
            total_steps: steps as u32,
            elapsed_secs: time,
            preview: None,
        });
    }
}

// ---------------------------------------------------------------------------
// SampleMethod → C enum conversion
// ---------------------------------------------------------------------------

impl SampleMethod {
    /// Map our Rust enum to the C `sample_method_t` value.
    pub(crate) fn to_c(self) -> sd_sys::sample_method_t {
        match self {
            SampleMethod::Euler => sd_sys::sample_method_t_EULER_SAMPLE_METHOD,
            SampleMethod::EulerA => sd_sys::sample_method_t_EULER_A_SAMPLE_METHOD,
            SampleMethod::Heun => sd_sys::sample_method_t_HEUN_SAMPLE_METHOD,
            SampleMethod::Dpm2 => sd_sys::sample_method_t_DPM2_SAMPLE_METHOD,
            SampleMethod::DpmPlusPlus2m => sd_sys::sample_method_t_DPMPP2M_SAMPLE_METHOD,
            SampleMethod::Lcm => sd_sys::sample_method_t_LCM_SAMPLE_METHOD,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- SampleMethod to C enum mapping --

    #[test]
    fn sample_method_euler_maps_correctly() {
        assert_eq!(SampleMethod::Euler.to_c(), sd_sys::sample_method_t_EULER_SAMPLE_METHOD);
    }

    #[test]
    fn sample_method_euler_a_maps_correctly() {
        assert_eq!(SampleMethod::EulerA.to_c(), sd_sys::sample_method_t_EULER_A_SAMPLE_METHOD);
    }

    #[test]
    fn sample_method_heun_maps_correctly() {
        assert_eq!(SampleMethod::Heun.to_c(), sd_sys::sample_method_t_HEUN_SAMPLE_METHOD);
    }

    #[test]
    fn sample_method_dpm2_maps_correctly() {
        assert_eq!(SampleMethod::Dpm2.to_c(), sd_sys::sample_method_t_DPM2_SAMPLE_METHOD);
    }

    #[test]
    fn sample_method_dpmpp2m_maps_correctly() {
        assert_eq!(
            SampleMethod::DpmPlusPlus2m.to_c(),
            sd_sys::sample_method_t_DPMPP2M_SAMPLE_METHOD
        );
    }

    #[test]
    fn sample_method_lcm_maps_correctly() {
        assert_eq!(SampleMethod::Lcm.to_c(), sd_sys::sample_method_t_LCM_SAMPLE_METHOD);
    }

    // -- ContextConfig with missing model returns ModelNotFound --

    #[test]
    fn new_context_with_missing_model_returns_error() {
        let config = ContextConfig {
            model_path: "/nonexistent/model.gguf".into(),
            vae_path: None,
            n_threads: 4,
        };
        let result = SdCppContext::new(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            SdError::ModelNotFound { path } => assert!(path.contains("nonexistent")),
            other => panic!("Expected ModelNotFound, got: {:?}", other),
        }
    }
}
