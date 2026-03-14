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
        // Validate that at least one model path is provided
        if config.model_path.is_none() && config.diffusion_model_path.is_none() {
            return Err(SdError::InvalidParams {
                reason: "No model path provided — set model_path (SD/SDXL) or diffusion_model_path (Flux)".into(),
            });
        }

        // Validate model_path exists if provided (SD1.5/SDXL)
        if let Some(ref mp) = config.model_path {
            if !std::path::Path::new(mp).exists() {
                return Err(SdError::ModelNotFound { path: mp.clone() });
            }
        }

        // Validate diffusion_model_path exists if provided (Flux)
        if let Some(ref dp) = config.diffusion_model_path {
            if !std::path::Path::new(dp).exists() {
                return Err(SdError::ModelNotFound { path: dp.clone() });
            }
        }

        // Build CStrings BEFORE the unsafe block so they stay alive through new_sd_ctx()
        let model_path_c = config.model_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "model_path contains interior NUL byte".into(),
            })?;

        let vae_path_c = config.vae_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "vae_path contains interior NUL byte".into(),
            })?;

        let clip_l_path_c = config.clip_l_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "clip_l_path contains interior NUL byte".into(),
            })?;

        let t5xxl_path_c = config.t5xxl_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "t5xxl_path contains interior NUL byte".into(),
            })?;

        let diffusion_model_path_c = config.diffusion_model_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "diffusion_model_path contains interior NUL byte".into(),
            })?;

        let llm_path_c = config.llm_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "llm_path contains interior NUL byte".into(),
            })?;

        unsafe {
            // Install sd.cpp log callback so we can see CUDA init, backend selection, etc.
            sd_sys::sd_set_log_callback(Some(sd_log_trampoline), std::ptr::null_mut());

            let mut params: sd_sys::sd_ctx_params_t = std::mem::zeroed();
            sd_sys::sd_ctx_params_init(&mut params);

            if let Some(ref mp) = model_path_c {
                params.model_path = mp.as_ptr();
            }
            if let Some(ref vae) = vae_path_c {
                params.vae_path = vae.as_ptr();
            }
            if let Some(ref clip_l) = clip_l_path_c {
                params.clip_l_path = clip_l.as_ptr();
            }
            if let Some(ref t5xxl) = t5xxl_path_c {
                params.t5xxl_path = t5xxl.as_ptr();
            }
            if let Some(ref diff) = diffusion_model_path_c {
                params.diffusion_model_path = diff.as_ptr();
            }
            if let Some(ref llm) = llm_path_c {
                params.llm_path = llm.as_ptr();
            }
            params.n_threads = config.n_threads;
            params.vae_decode_only = true;
            // SD_TYPE_COUNT = auto-detect quantization from model file
            params.wtype = sd_sys::sd_type_t_SD_TYPE_COUNT;

            // Performance settings
            params.flash_attn = config.flash_attn;
            params.diffusion_flash_attn = config.diffusion_flash_attn;
            params.enable_mmap = config.enable_mmap;
            params.free_params_immediately = config.free_params_immediately;
            params.keep_clip_on_cpu = config.keep_clip_on_cpu;
            params.keep_vae_on_cpu = config.keep_vae_on_cpu;
            params.offload_params_to_cpu = config.offload_params_to_cpu;

            let model_display = config.model_path.as_deref()
                .or(config.diffusion_model_path.as_deref())
                .unwrap_or("<none>");
            log::info!(
                "Creating sd.cpp context: model={}, threads={}",
                model_display,
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
        let mut decoded_rgb: Vec<u8>;
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

        // Set up progress trampoline — heap-allocated so it outlives any early return
        // from generate() while sd.cpp's global callback is still set.
        let trampoline_data = Box::new(ProgressTrampolineData {
            callback: progress_cb,
        });
        let trampoline_ptr = Box::into_raw(trampoline_data);

        unsafe {
            // Install progress callback (GLOBAL in sd.cpp)
            sd_sys::sd_set_progress_callback(
                Some(progress_trampoline),
                trampoline_ptr as *mut c_void,
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

            let user_method = params.sample_method.to_c();
            sample_params.sample_method = user_method;
            sample_params.sample_steps = params.steps as i32;
            sample_params.guidance.txt_cfg = params.cfg_scale;

            // Karras scheduler works for SD/SDXL. For flow models (Flux/Z-Image),
            // use sd.cpp's model-aware default instead.
            let default_method = sd_sys::sd_get_default_sample_method(self.ctx);
            let model_scheduler = sd_sys::sd_get_default_scheduler(self.ctx, default_method);
            // If the model's default scheduler differs from Karras, it's a flow model — use its default
            if model_scheduler != sd_sys::scheduler_t_KARRAS_SCHEDULER {
                sample_params.scheduler = model_scheduler;
            } else {
                sample_params.scheduler = sd_sys::scheduler_t_KARRAS_SCHEDULER;
            }

            gen_params.sample_params = sample_params;

            eprintln!("[blink] sample_method={}, scheduler={}, steps={}, cfg={}",
                user_method, sample_params.scheduler, params.steps, params.cfg_scale);

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

            // Clear progress callback immediately, then reclaim the boxed trampoline data.
            // Must happen in this order: clear first so sd.cpp can no longer call into it,
            // then drop the box.
            sd_sys::sd_set_progress_callback(None, std::ptr::null_mut());
            let _ = Box::from_raw(trampoline_ptr);

            if result_ptr.is_null() {
                return Err(SdError::InferenceReturnedNull);
            }

            // Read the first image (batch_count = 1)
            let sd_img = &*result_ptr;
            let w = sd_img.width;
            let h = sd_img.height;
            let ch = sd_img.channel;
            let pixel_count = (w as usize) * (h as usize) * (ch as usize);

            eprintln!("[blink] Image result: {}x{}, {} channels, data_null={}, pixel_count={}, data_ptr={:?}",
                w, h, ch, sd_img.data.is_null(), pixel_count, sd_img.data);

            if sd_img.data.is_null() || pixel_count == 0 {
                libc::free(result_ptr as *mut c_void);
                return Err(SdError::InferenceReturnedNull);
            }

            // Copy pixel data out before freeing
            let src_slice = std::slice::from_raw_parts(sd_img.data, pixel_count);

            // Log image data diagnostics
            let sample_start: Vec<u8> = src_slice.iter().take(24).copied().collect();
            let mid = pixel_count / 2;
            let sample_mid: Vec<u8> = src_slice[mid..].iter().take(24).copied().collect();
            let nonzero = src_slice.iter().filter(|&&b| b != 0).count();
            eprintln!("[blink] First 24 bytes: {:?}", sample_start);
            eprintln!("[blink] Mid 24 bytes:   {:?}", sample_mid);
            eprintln!("[blink] non-zero: {}/{}", nonzero, pixel_count);
            // Check if data might be float (look for IEEE 754 patterns)
            if pixel_count >= 4 {
                let f32_bytes: [u8; 4] = [src_slice[0], src_slice[1], src_slice[2], src_slice[3]];
                let as_f32 = f32::from_le_bytes(f32_bytes);
                eprintln!("[blink] First 4 bytes as f32: {}", as_f32);
            }

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

            // sd.cpp allocates the sd_image_t array with new[] and the pixel data with malloc().
            // Both must be freed separately. Verified against sd.cpp source: generate_image()
            // in stable-diffusion.cpp allocates `data` via stbi_write_png_to_mem / malloc,
            // and the result array itself via operator new[].
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
// sd.cpp log callback — prints to stderr so it's visible in terminal
// ---------------------------------------------------------------------------

unsafe extern "C" fn sd_log_trampoline(
    level: sd_sys::sd_log_level_t,
    text: *const std::os::raw::c_char,
    _data: *mut c_void,
) {
    if text.is_null() {
        return;
    }
    let msg = std::ffi::CStr::from_ptr(text).to_string_lossy();
    let trimmed = msg.trim_end();
    if !trimmed.is_empty() {
        let prefix = match level {
            sd_sys::sd_log_level_t_SD_LOG_ERROR => "[sd.cpp ERROR]",
            sd_sys::sd_log_level_t_SD_LOG_WARN => "[sd.cpp WARN]",
            sd_sys::sd_log_level_t_SD_LOG_INFO => "[sd.cpp]",
            _ => "[sd.cpp DEBUG]",
        };
        eprintln!("{} {}", prefix, trimmed);
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
    eprintln!("[blink] Progress: step={}, total_steps={}, time={:.1}s", step, steps, time);
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
            model_path: Some("/nonexistent/model.gguf".into()),
            vae_path: None,
            clip_l_path: None,
            t5xxl_path: None,
            diffusion_model_path: None,
            llm_path: None,
            n_threads: 4,
            flash_attn: false,
            diffusion_flash_attn: false,
            enable_mmap: true,
            free_params_immediately: false,
            keep_clip_on_cpu: false,
            keep_vae_on_cpu: false,
            offload_params_to_cpu: false,
        };
        let result = SdCppContext::new(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            SdError::ModelNotFound { path } => assert!(path.contains("nonexistent")),
            other => panic!("Expected ModelNotFound, got: {:?}", other),
        }
    }
}
