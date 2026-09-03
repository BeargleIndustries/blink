//! Safe FFI bridge over raw sd.cpp bindings.
//!
//! All `unsafe` interactions with sd-sys live here. The rest of sd-wrapper
//! treats `SdCppContext` as a safe, Send-able handle.

use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::SdError;
use crate::progress::{ProgressCallback, ProgressUpdate};
use crate::types::*;

/// Callback type for live preview frames during generation.
/// Arguments: step, RGBA pixel data, width, height
pub type PreviewCallback = Box<dyn Fn(i32, Vec<u8>, u32, u32) + Send>;

/// The raw context pointer, made `Send` so a cancel issued from the UI thread can
/// reach it. It is never dereferenced by Rust — only handed back to sd.cpp.
struct CtxPtr(*mut sd_sys::sd_ctx_t);
unsafe impl Send for CtxPtr {}

/// Lets a thread other than the inference thread call sd.cpp's native cancel
/// without racing `free_sd_ctx`.
///
/// `sd_cancel_generation` is safe to call cross-thread: its whole body is a null
/// check, a range clamp and a release-store into a lock-free atomic
/// (`stable-diffusion.cpp:3893`, flag declared `:290`). The only hazard is the
/// context being freed underneath it, and the mutex closes exactly that: `clear()`
/// runs before `free_sd_ctx` and cannot acquire the lock while a cancel is in
/// flight, so a freed pointer is never visible to `cancel()`. Holding the lock
/// across the FFI call is cheap and cannot deadlock — a single atomic store
/// neither blocks nor re-enters.
///
/// There is deliberately no per-generation `reset()`: `generate_image` already
/// clears sd.cpp's flag itself at `stable-diffusion.cpp:5644`, so issuing
/// `SD_CANCEL_RESET` here would be dead code. `pending` is the only state Blink
/// owns, and it is cleared only in `set()`.
pub struct CancelHandle {
    /// `None` before `new_sd_ctx` returns and after the context is freed.
    ctx: Mutex<Option<CtxPtr>>,
    /// A cancel that arrived before the context existed, replayed by `set()`.
    pending: AtomicBool,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self {
            ctx: Mutex::new(None),
            pending: AtomicBool::new(false),
        }
    }

    /// Publish the context pointer. Called on the inference thread immediately
    /// after a successful `new_sd_ctx`. Replays a cancel that arrived first.
    fn set(&self, ptr: *mut sd_sys::sd_ctx_t) {
        let mut guard = match self.ctx.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(CtxPtr(ptr));
        if self.pending.swap(false, Ordering::SeqCst) {
            log::info!("Replaying a cancel that arrived before the model finished loading");
            unsafe { sd_sys::sd_cancel_generation(ptr, sd_sys::sd_cancel_mode_t_SD_CANCEL_ALL) };
        }
    }

    /// Retract the pointer. Called on the inference thread *before* `free_sd_ctx`.
    fn clear(&self) {
        let mut guard = match self.ctx.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }

    /// Ask sd.cpp to stop at its next check point. Safe from any thread.
    ///
    /// This cannot abort a model load: every `get_cancel_flag()` call site is in
    /// the generate/decode paths and `model_loader.cpp` has none, so a cancel
    /// issued during loading takes effect at the first sampling step instead.
    pub fn cancel(&self) {
        let guard = match self.ctx.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.as_ref() {
            Some(p) => unsafe {
                sd_sys::sd_cancel_generation(p.0, sd_sys::sd_cancel_mode_t_SD_CANCEL_ALL)
            },
            None => self.pending.store(true, Ordering::SeqCst),
        }
    }
}

impl Default for CancelHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Thin owning wrapper around `*mut sd_sys::sd_ctx_t`.
/// Freed on drop via `sd_sys::free_sd_ctx`.
pub(crate) struct SdCppContext {
    ctx: *mut sd_sys::sd_ctx_t,
    cancel_handle: Arc<CancelHandle>,
}

// sd_ctx_t is internally synchronized by sd.cpp (single-threaded use from our
// dedicated inference thread), so Send is sound.
unsafe impl Send for SdCppContext {}

impl SdCppContext {
    /// Return the raw context pointer for FFI calls that need it directly (e.g. `generate_video`).
    /// Safety: valid as long as this `SdCppContext` is alive.
    pub(crate) fn raw_ptr(&self) -> *mut sd_sys::sd_ctx_t {
        self.ctx
    }

        /// Load a model and create the sd.cpp context.
    ///
    /// `cancel_handle` is published as soon as `new_sd_ctx` returns, so a cancel
    /// pressed during the load is replayed against the fresh context rather than
    /// dropped.
    ///
    /// `load_progress_cb` is installed around `new_sd_ctx` with
    /// `expected_steps == 0`, so switching models is not a dead UI. Its events are
    /// tensor counts; the phase machine defers to the log markers there.
    pub(crate) fn new(
        config: &ContextConfig,
        cancel_handle: &Arc<CancelHandle>,
        load_progress_cb: Option<ProgressCallback>,
    ) -> Result<Self, SdError> {
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

        let control_net_path_c = config.control_net_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "control_net_path contains interior NUL byte".into(),
            })?;

        let taesd_path_c = config.taesd_path.as_ref()
            .map(|p| CString::new(p.as_str()))
            .transpose()
            .map_err(|_| SdError::InvalidParams {
                reason: "taesd_path contains interior NUL byte".into(),
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
            if let Some(ref cn) = control_net_path_c {
                params.control_net_path = cn.as_ptr();
            }
            if let Some(ref taesd) = taesd_path_c {
                params.taesd_path = taesd.as_ptr();
            }
            params.n_threads = config.n_threads;
            // `vae_decode_only` was removed from the C API; sd.cpp now always keeps
            // the encoder available, which is what img2img needs anyway.
            // SD_TYPE_COUNT = auto-detect quantization from model file
            params.wtype = sd_sys::sd_type_t_SD_TYPE_COUNT;

            // Performance settings. These were user toggles until parameter
            // placement moved to sd.cpp's auto_fit; the values below are the ones
            // PerfSettings::default() shipped, so behaviour is unchanged. They are
            // sd.cpp internals a non-technical user cannot reason about, so they
            // are no longer exposed in the UI.
            params.flash_attn = true;
            params.diffusion_flash_attn = true;
            params.enable_mmap = true;

            params.lora_apply_mode = match config.lora_apply_mode {
                LoraApplyMode::Auto => sd_sys::lora_apply_mode_t_LORA_APPLY_AUTO,
                LoraApplyMode::Immediately => sd_sys::lora_apply_mode_t_LORA_APPLY_IMMEDIATELY,
                LoraApplyMode::AtRuntime => sd_sys::lora_apply_mode_t_LORA_APPLY_AT_RUNTIME,
            };

            // Parameter placement. `low_memory` and `auto_fit` are mutually
            // exclusive by construction: sd.cpp's derive_backend_specs() overwrites
            // params_backend whenever auto_fit is set, and warns that it is doing so.
            //
            // The `c"cpu"` literal has 'static lifetime, so the pointer sd.cpp
            // borrows stays valid across new_sd_ctx() without any keep-alive dance.
            if config.low_memory {
                // Known-good pre-auto_fit placement: params in CPU RAM, compute on GPU.
                // Measured ~2.4x faster than GPU-resident on a 12 GB card (research §4b).
                params.params_backend = c"cpu".as_ptr();
                log::info!("Parameter placement: low-memory mode (params_backend=cpu)");
            } else if config.uses_auto_fit() {
                params.auto_fit = true;
                log::info!("Parameter placement: auto_fit (sd.cpp decides from measured VRAM)");
            }

            let model_display = config.model_path.as_deref()
                .or(config.diffusion_model_path.as_deref())
                .unwrap_or("<none>");
            log::info!(
                "Creating sd.cpp context: model={}, threads={}",
                model_display,
                config.n_threads
            );

            // Progress during the load itself. `expected_steps: 0` tells the
            // phase machine there is no step count to compare against here, so
            // it defers to the log markers — which `reset_phase()` has already
            // put in `Loading`.
            crate::progress::reset_phase();
            let load_trampoline_ptr = Box::into_raw(Box::new(ProgressTrampolineData {
                callback: load_progress_cb,
                expected_steps: 0,
            }));
            sd_sys::sd_set_progress_callback(
                Some(progress_trampoline),
                load_trampoline_ptr as *mut c_void,
            );

            let ctx = sd_sys::new_sd_ctx(&params);

            // Same clear-then-reclaim ordering the generate path uses, and it
            // runs before the null check so the error path cannot leak the box
            // or leave sd.cpp holding a callback into it.
            sd_sys::sd_set_progress_callback(None, std::ptr::null_mut());
            let _ = Box::from_raw(load_trampoline_ptr);

            if ctx.is_null() {
                return Err(SdError::ContextCreationFailed {
                    reason: "new_sd_ctx returned null — model may be corrupted or incompatible"
                        .into(),
                });
            }

            // Publish before returning: a cancel pressed during the load is
            // parked in `pending` and replayed here.
            cancel_handle.set(ctx);

            Ok(Self {
                ctx,
                cancel_handle: Arc::clone(cancel_handle),
            })
        }
    }

    /// Run image generation (txt2img when `input_image` is `None`, img2img otherwise).
    pub(crate) fn generate(
        &self,
        params: &GenerationParams,
        input_image: Option<&[u8]>,
        mask_image: Option<&[u8]>,
        strength: f32,
        progress_cb: Option<ProgressCallback>,
        preview_cb: Option<PreviewCallback>,
        cancel_flag: &AtomicBool,
        ref_images: Option<&[Vec<u8>]>,
        control_image: Option<&[u8]>,
        control_strength: Option<f32>,
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

        // For img2img, sd.cpp aligns width/height UP to a spatial_multiple (e.g. 64 for SD1.5).
        // The init_image MUST match the ALIGNED dimensions, not the original.
        // We resize the input image to the aligned size to avoid the assert failure.
        let mut img2img_width = params.width;
        let mut img2img_height = params.height;

        if let Some(img_bytes) = input_image {
            let img = image::load_from_memory(img_bytes).map_err(|e| SdError::InvalidParams {
                reason: format!("Failed to decode input image: {}", e),
            })?;
            // Align dimensions up to multiple of 64 (covers all model architectures:
            // SD1.5=64, SDXL=64, Flux=64, Z-Image=64)
            let (orig_w, orig_h) = (img.width(), img.height());
            let align = 64u32;
            let aligned_w = ((orig_w + align - 1) / align) * align;
            let aligned_h = ((orig_h + align - 1) / align) * align;

            let resized = img.resize_exact(aligned_w, aligned_h, image::imageops::FilterType::Lanczos3);
            let rgb_img = resized.to_rgb8();
            let (w, h) = rgb_img.dimensions();
            eprintln!("[blink] img2img input: {}x{} -> aligned {}x{}", orig_w, orig_h, w, h);
            decoded_rgb = rgb_img.into_raw();
            init_image = sd_sys::sd_image_t {
                width: w,
                height: h,
                channel: 3,
                data: decoded_rgb.as_mut_ptr(),
            };
            img2img_width = w;
            img2img_height = h;
        }

        // --- Decode ref images for Kontext ---
        let mut ref_rgb_bufs: Vec<Vec<u8>> = Vec::new();
        let mut ref_sd_images: Vec<sd_sys::sd_image_t> = Vec::new();
        if let Some(ref_imgs) = ref_images {
            for (i, img_bytes) in ref_imgs.iter().enumerate() {
                let img = image::load_from_memory(img_bytes).map_err(|e| SdError::InvalidParams {
                    reason: format!("Failed to decode ref_image[{}]: {}", i, e),
                })?;
                let rgb = img.to_rgb8();
                let (w, h) = rgb.dimensions();
                let mut raw = rgb.into_raw();
                ref_sd_images.push(sd_sys::sd_image_t {
                    width: w,
                    height: h,
                    channel: 3,
                    data: raw.as_mut_ptr(),
                });
                ref_rgb_bufs.push(raw);
            }
        }

        // --- Build LoRA structs ---
        let lora_path_cstrings: Vec<CString> = params.loras.iter()
            .map(|l| CString::new(l.path.as_str()).map_err(|_| SdError::InvalidParams {
                reason: format!("LoRA path contains interior NUL byte: {}", l.path),
            }))
            .collect::<Result<Vec<_>, _>>()?;
        let lora_structs: Vec<sd_sys::sd_lora_t> = params.loras.iter()
            .zip(lora_path_cstrings.iter())
            .map(|(lora, cpath)| sd_sys::sd_lora_t {
                is_high_noise: lora.is_high_noise,
                multiplier: lora.multiplier,
                path: cpath.as_ptr(),
            })
            .collect();

        // --- Decode control image ---
        #[allow(unused_assignments)]
        let mut control_rgb: Vec<u8> = Vec::new();
        let mut control_sd_image = sd_sys::sd_image_t {
            width: 0,
            height: 0,
            channel: 0,
            data: std::ptr::null_mut(),
        };
        if let Some(ctrl_bytes) = control_image {
            let img = image::load_from_memory(ctrl_bytes).map_err(|e| SdError::InvalidParams {
                reason: format!("Failed to decode control image: {}", e),
            })?;
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            control_rgb = rgb.into_raw();
            control_sd_image = sd_sys::sd_image_t {
                width: w,
                height: h,
                channel: 3,
                data: control_rgb.as_mut_ptr(),
            };
        }

        // Set up progress trampoline — heap-allocated so it outlives any early return
        // from generate() while sd.cpp's global callback is still set.
        // Start every generation in `Loading` with the latch clear, so a lazy
        // weight load inside this generation is labelled honestly.
        crate::progress::reset_phase();

        let trampoline_data = Box::new(ProgressTrampolineData {
            callback: progress_cb,
            // The value Blink REQUESTED, read before sd.cpp sees it — not
            // re-read from gen_params afterwards.
            expected_steps: params.steps,
        });
        let trampoline_ptr = Box::into_raw(trampoline_data);

        // Set up preview trampoline if callback provided
        let preview_trampoline_ptr = if let Some(pcb) = preview_cb {
            let data = Box::new(PreviewTrampolineData { callback: pcb });
            Box::into_raw(data)
        } else {
            std::ptr::null_mut()
        };

        unsafe {
            // Install progress callback (GLOBAL in sd.cpp)
            sd_sys::sd_set_progress_callback(
                Some(progress_trampoline),
                trampoline_ptr as *mut c_void,
            );

            // Install preview callback if requested
            if !preview_trampoline_ptr.is_null() {
                sd_sys::sd_set_preview_callback(
                    Some(preview_trampoline),
                    sd_sys::preview_t_PREVIEW_TAE, // TAE preview mode
                    3,     // interval: every 3 steps
                    true,  // denoised
                    false, // noisy
                    preview_trampoline_ptr as *mut c_void,
                );
            }

            // Callback teardown, shared by the cancel path below and the normal
            // path after generate_image, so the two cannot drift apart. Order is
            // load-bearing: clear first so sd.cpp can no longer call into the
            // boxes, then reclaim them.
            let teardown = || {
                sd_sys::sd_set_progress_callback(None, std::ptr::null_mut());
                let _ = Box::from_raw(trampoline_ptr);
                if !preview_trampoline_ptr.is_null() {
                    sd_sys::sd_set_preview_callback(
                        None,
                        sd_sys::preview_t_PREVIEW_NONE,
                        0,
                        false,
                        false,
                        std::ptr::null_mut(),
                    );
                    let _ = Box::from_raw(preview_trampoline_ptr);
                }
            };

            // Build generation params
            let mut gen_params: sd_sys::sd_img_gen_params_t = std::mem::zeroed();
            sd_sys::sd_img_gen_params_init(&mut gen_params);

            // Inference caching. `sd_cache_params_init` fills in the per-mode
            // defaults, including all seven `spectrum_*` fields — Blink sets the
            // mode and nothing else, and exposes no cache mode or parameter to
            // the user. Off by construction when `fast_mode` is false:
            // `SD_CACHE_DISABLED` is 0, which is what the zeroed struct already
            // held before this existed.
            sd_sys::sd_cache_params_init(&mut gen_params.cache);
            gen_params.cache.mode = cache_mode_for(params.fast_mode);

            gen_params.prompt = prompt_c.as_ptr();
            gen_params.negative_prompt = neg_prompt_c.as_ptr();
            gen_params.width = img2img_width as i32;
            gen_params.height = img2img_height as i32;
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

            // Image conditioning strength (Kontext)
            if let Some(img_cfg) = params.img_cfg {
                sample_params.guidance.img_cfg = img_cfg;
            }

            gen_params.sample_params = sample_params;

            // Ref images (Kontext editing)
            if !ref_sd_images.is_empty() {
                gen_params.ref_images = ref_sd_images.as_mut_ptr();
                gen_params.ref_images_count = ref_sd_images.len() as i32;
                // `auto_resize_ref_image` / `increase_ref_index` were folded into the
                // `ref_image_args` key-value string. Auto-resize is expressed there as
                // `resize_before_vae`, which already defaults to true — the old value we
                // set — so leaving ref_image_args empty preserves the previous behaviour
                // and lets sd.cpp apply its per-model reference-image preset.
            }

            // LoRA adapters
            if !lora_structs.is_empty() {
                gen_params.loras = lora_structs.as_ptr();
                gen_params.lora_count = lora_structs.len() as u32;
            }

            // ControlNet
            if control_image.is_some() {
                gen_params.control_image = control_sd_image;
                gen_params.control_strength = control_strength.unwrap_or(0.9);
            }

            eprintln!("[blink] sample_method={}, scheduler={}, steps={}, cfg={}",
                user_method, sample_params.scheduler, params.steps, params.cfg_scale);

            // img2img: build mask before the generate call so it stays alive
            let mut mask_data: Vec<u8> = if input_image.is_some() {
                vec![255u8; (img2img_width * img2img_height) as usize]
            } else {
                Vec::new()
            };

            if input_image.is_some() {
                // If a mask was provided, decode and resize it to match the aligned dimensions
                if let Some(mask_bytes) = mask_image {
                    let mask_img = image::load_from_memory(mask_bytes).map_err(|e| SdError::InvalidParams {
                        reason: format!("Failed to decode mask image: {}", e),
                    })?;
                    let resized_mask = mask_img.resize_exact(img2img_width, img2img_height, image::imageops::FilterType::Nearest);
                    mask_data = resized_mask.to_luma8().into_raw();
                }

                gen_params.init_image = init_image;
                gen_params.strength = strength;
                // mask_image must have matching dimensions — sd.cpp asserts width/height/channel
                // before reading data. All-255 mask = "modify everything" (no inpainting).
                gen_params.mask_image = sd_sys::sd_image_t {
                    width: img2img_width,
                    height: img2img_height,
                    channel: 1,
                    data: mask_data.as_mut_ptr(),
                };
            } else {
                gen_params.strength = 0.0;
            }

            // Test-only: widen the pre-flight window so a cancel can be delivered
            // deterministically instead of raced. `debug_assertions` (not
            // `cfg(test)`, which would not apply to this library crate when the
            // integration test runs) keeps it out of release builds entirely,
            // while `cargo test`'s dev profile still exercises it.
            #[cfg(debug_assertions)]
            if let Ok(ms) = std::env::var("BLINK_TEST_PREFLIGHT_DELAY_MS") {
                if let Ok(ms) = ms.parse::<u64>() {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
            }

            // Second cancel read, immediately before handing control to sd.cpp.
            // Everything since the pre-flight check at the top of generate() — the
            // Lanczos resize, the LoRA CString build, the mask allocation — is a
            // window in which a cancel would otherwise be silently discarded by
            // generate_image's own reset_cancel_flag() (stable-diffusion.cpp:5644)
            // and the user would receive an image they asked not to have.
            if cancel_flag.load(Ordering::SeqCst) {
                log::info!("Cancelled before generate_image; tearing down callbacks");
                teardown();
                return Err(SdError::Cancelled);
            }

            log::info!(
                "Calling generate_image: prompt='{}', {}x{}, {} steps, seed={}",
                params.prompt,
                params.width,
                params.height,
                params.steps,
                params.seed
            );

            // generate_image() now returns a bool and writes results through out-params.
            let mut result_ptr: *mut sd_sys::sd_image_t = std::ptr::null_mut();
            let mut num_images_out: std::ffi::c_int = 0;
            let ok = sd_sys::generate_image(
                self.ctx,
                &gen_params,
                &mut result_ptr,
                &mut num_images_out,
            );

            // Clear callbacks immediately, then reclaim the boxed trampoline data.
            teardown();

            if !ok || result_ptr.is_null() || num_images_out <= 0 {
                if !result_ptr.is_null() {
                    sd_sys::free_sd_images(result_ptr, num_images_out);
                }
                // generate_image returns false on cancel (stable-diffusion.cpp:5687,
                // "cancelling generation"). Without this the user is told
                // "inference returned null" for something they asked for.
                if cancel_flag.load(Ordering::SeqCst) {
                    return Err(SdError::Cancelled);
                }
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
                sd_sys::free_sd_images(result_ptr, num_images_out);
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

            // free_sd_images releases both the pixel buffers and the array. sd.cpp added
            // it specifically so callers stop freeing library-allocated memory themselves:
            // on Windows the library and this crate may link different CRTs, and the old
            // libc::free() path could corrupt the heap.
            sd_sys::free_sd_images(result_ptr, num_images_out);

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
        // Retract the pointer BEFORE freeing it. `clear()` blocks until any
        // in-flight `cancel()` has released the lock, so sd.cpp can never be
        // handed a freed context.
        self.cancel_handle.clear();
        if !self.ctx.is_null() {
            log::info!("Freeing sd.cpp context");
            unsafe {
                sd_sys::free_sd_ctx(self.ctx);
            }
        }
    }
}

/// Which sd.cpp cache mode a `fast_mode` flag selects.
///
/// Only `spectrum` is in scope: it is the one mode that works on both UNet and
/// DiT architectures. The other five (`easycache`, `ucache`, `dbcache`,
/// `taylorseer`, `cache_dit`) are deliberately unreachable — a cache mode is an
/// engine internal, not a user-facing choice.
pub(crate) fn cache_mode_for(fast_mode: bool) -> sd_sys::sd_cache_mode_t {
    if fast_mode {
        sd_sys::sd_cache_mode_t_SD_CACHE_SPECTRUM
    } else {
        sd_sys::sd_cache_mode_t_SD_CACHE_DISABLED
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
        // One-way phase upgrades. This is the only signal available during the
        // `new_sd_ctx` window, where no step count exists to reason from.
        crate::progress::note_log_line(trimmed);
    }
}

// ---------------------------------------------------------------------------
// Progress callback trampoline
// ---------------------------------------------------------------------------

struct ProgressTrampolineData {
    callback: Option<ProgressCallback>,
    /// The step count Blink asked for, or 0 for the `new_sd_ctx` window.
    expected_steps: u32,
}

/// C-compatible callback forwarded from sd.cpp's global progress system.
unsafe extern "C" fn progress_trampoline(step: c_int, steps: c_int, time: f32, data: *mut c_void) {
    if data.is_null() {
        eprintln!("[blink] Progress: step={}, total_steps={}, time={:.1}s", step, steps, time);
        return;
    }
    let cb_data = &*(data as *const ProgressTrampolineData);
    // The heuristic runs UNCONDITIONALLY on every event — it is not gated on
    // "no marker seen yet". Markers influence the result only by having already
    // upgraded the stored phase through `note_log_line`.
    let phase = crate::progress::note_progress(
        step as u32,
        steps as u32,
        cb_data.expected_steps,
    );
    eprintln!(
        "[blink] Progress: step={}, total_steps={}, time={:.1}s, phase={}",
        step,
        steps,
        time,
        phase.as_str()
    );
    if let Some(ref cb) = cb_data.callback {
        cb(ProgressUpdate {
            step: step as u32,
            total_steps: steps as u32,
            elapsed_secs: time,
            preview: None,
            phase,
        });
    }
}

// ---------------------------------------------------------------------------
// Preview callback trampoline
// ---------------------------------------------------------------------------

struct PreviewTrampolineData {
    callback: PreviewCallback,
}

/// C-compatible callback forwarded from sd.cpp's global preview system.
unsafe extern "C" fn preview_trampoline(
    step: c_int,
    frame_count: c_int,
    frames: *mut sd_sys::sd_image_t,
    _is_noisy: bool,
    data: *mut c_void,
) {
    if data.is_null() || frames.is_null() || frame_count < 1 {
        return;
    }
    let cb_data = &*(data as *const PreviewTrampolineData);
    let frame = &*frames; // take first frame
    let w = frame.width;
    let h = frame.height;
    let ch = frame.channel as usize;
    if frame.data.is_null() || w == 0 || h == 0 {
        return;
    }
    let pixel_count = (w as usize) * (h as usize) * ch;
    let src = std::slice::from_raw_parts(frame.data, pixel_count);

    // Convert to RGBA
    let rgba = if ch == 3 {
        let mut buf = Vec::with_capacity((w as usize) * (h as usize) * 4);
        for pixel in src.chunks_exact(3) {
            buf.push(pixel[0]);
            buf.push(pixel[1]);
            buf.push(pixel[2]);
            buf.push(255);
        }
        buf
    } else {
        src.to_vec()
    };

    (cb_data.callback)(step, rgba, w, h);
}

// ---------------------------------------------------------------------------
// Canny edge detection preprocessor
// ---------------------------------------------------------------------------

/// Preprocess an image with Canny edge detection for ControlNet.
/// Returns PNG-encoded edge map.
pub fn preprocess_canny(
    image_data: &[u8],
    high_threshold: f32,
    low_threshold: f32,
    weak: f32,
    strong: f32,
    inverse: bool,
) -> Result<Vec<u8>, SdError> {
    let img = image::load_from_memory(image_data).map_err(|e| SdError::InvalidParams {
        reason: format!("Failed to decode image for canny preprocessing: {}", e),
    })?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut raw = rgb.into_raw();

    let sd_img = sd_sys::sd_image_t {
        width: w,
        height: h,
        channel: 3,
        data: raw.as_mut_ptr(),
    };

    let success = unsafe {
        sd_sys::preprocess_canny(sd_img, high_threshold, low_threshold, weak, strong, inverse)
    };

    if !success {
        return Err(SdError::InvalidParams {
            reason: "Canny edge detection preprocessing failed".into(),
        });
    }

    // Encode the modified image data as PNG
    use image::{ImageBuffer, Rgb};
    let out_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, raw)
        .ok_or_else(|| SdError::InvalidParams {
            reason: "Failed to create image buffer from canny output".into(),
        })?;
    let mut png_bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    out_img.write_to(&mut cursor, image::ImageFormat::Png).map_err(|e| SdError::InvalidParams {
        reason: format!("Failed to encode canny output as PNG: {}", e),
    })?;

    Ok(png_bytes)
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

    // -- Inference cache --

    #[test]
    fn cache_is_disabled_when_fast_mode_is_off() {
        // The off path must be inert: SD_CACHE_DISABLED is 0, the same value the
        // zeroed `sd_img_gen_params_t` carried before caching existed, which is
        // what acceptance criterion 1 (byte-identical output) rests on.
        assert_eq!(cache_mode_for(false), sd_sys::sd_cache_mode_t_SD_CACHE_DISABLED);
        assert_eq!(sd_sys::sd_cache_mode_t_SD_CACHE_DISABLED, 0);
    }

    #[test]
    fn fast_mode_selects_spectrum_and_nothing_else() {
        assert_eq!(cache_mode_for(true), sd_sys::sd_cache_mode_t_SD_CACHE_SPECTRUM);
        // Guard against a silent renumbering of the enum putting us on a
        // different mode: spectrum is the only one in scope.
        assert_ne!(
            sd_sys::sd_cache_mode_t_SD_CACHE_SPECTRUM,
            sd_sys::sd_cache_mode_t_SD_CACHE_TAYLORSEER
        );
    }

    #[test]
    fn generation_params_default_to_fast_mode_off() {
        assert!(!GenerationParams::default().fast_mode);
    }

    // -- CancelHandle --
    //
    // These exercise the states that do NOT touch FFI. A handle whose `ctx` is
    // `None` never calls sd.cpp, so both tests are pure Rust and run on CI.

    #[test]
    fn cancel_handle_records_pending_before_ctx_is_set() {
        let handle = CancelHandle::new();
        // No context yet — the cancel must be parked, not dropped.
        handle.cancel();
        assert!(
            handle.pending.load(Ordering::SeqCst),
            "a cancel issued before new_sd_ctx returned must be remembered"
        );
    }

    #[test]
    fn cancel_handle_is_inert_after_clear() {
        let handle = CancelHandle::new();
        handle.clear();
        // clear() leaves ctx None, so this parks rather than dereferencing a
        // freed pointer. The point is that it does not crash and does not call in.
        handle.cancel();
        assert!(handle.ctx.lock().unwrap().is_none());
        assert!(handle.pending.load(Ordering::SeqCst));
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
            auto_fit: true,
            low_memory: false,
            control_net_path: None,
            taesd_path: None,
            lora_apply_mode: LoraApplyMode::Auto,
        };
        // Match on the Result directly rather than calling unwrap_err(): that
        // requires the Ok type to implement Debug, and SdCppContext deliberately
        // does not (it wraps a raw sd_ctx_t pointer).
        let handle = Arc::new(CancelHandle::new());
        match SdCppContext::new(&config, &handle, None) {
            Err(SdError::ModelNotFound { path }) => assert!(path.contains("nonexistent")),
            Err(other) => panic!("Expected ModelNotFound, got: {:?}", other),
            Ok(_) => panic!("Expected ModelNotFound, got a context"),
        }
    }
}
