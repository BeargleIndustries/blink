use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use sd_wrapper::{SdContext, ContextConfig, UpscalerContext, LoraApplyMode, CancelHandle};
use serde::{Deserialize, Serialize};

/// The user's only remaining placement choice. Everything else sd.cpp decides
/// for itself from measured VRAM (`ContextConfig::auto_fit`).
///
/// `#[serde(default)]` is load-bearing for the migration: a `settings.json`
/// written by an older build has none of these keys, and the read path in
/// `models.rs` is `serde_json::from_value(...).ok().unwrap_or_default()`, which
/// swallows a deserialization failure and makes it indistinguishable from "no
/// settings stored". Without the attribute the user's choice would be silently
/// discarded rather than erroring. Covered by
/// `perf_settings_deserializes_legacy_json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerfSettings {
    #[serde(default)]
    pub low_memory: bool,
}

/// Progress while a model is being read off disk. Emitted as
/// `model:load_progress`; the counts are tensors, which the UI must render as a
/// percentage and never as a number.
#[derive(Debug, Serialize, Clone)]
pub struct ModelLoadProgressEvent {
    pub step: u32,
    pub total_steps: u32,
}

pub struct AppState {
    pub app_handle: AppHandle,
    pub cancel_flag: Arc<AtomicBool>,
    /// Native sd.cpp cancellation. Owned here, alongside `cancel_flag`, for the
    /// same reason: `cancel_generation` must not lock `sd_context`, which the
    /// generation command holds for the whole generation.
    pub cancel_handle: Arc<CancelHandle>,
    pub active_model: Mutex<Option<String>>,
    pub generating: AtomicBool,
    pub sd_context: Mutex<Option<SdContext>>,
    pub model_dir: Mutex<String>,
    pub upscaler: Mutex<Option<UpscalerContext>>,
}

pub struct ModelPaths {
    pub model_path: Option<String>,
    pub vae_path: Option<String>,
    pub clip_l_path: Option<String>,
    pub t5xxl_path: Option<String>,
    pub diffusion_model_path: Option<String>,
    pub llm_path: Option<String>,
    pub control_net_path: Option<String>,
    pub taesd_path: Option<String>,
}

impl AppState {
    pub fn new(app_handle: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        // Get platform-appropriate model directory
        let app_data = app_handle.path().app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let model_dir = app_data.join("models");
        std::fs::create_dir_all(&model_dir)?;

        // Also create gallery directory
        let gallery_dir = app_data.join("gallery");
        std::fs::create_dir_all(&gallery_dir)?;

        Ok(Self {
            app_handle,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            cancel_handle: Arc::new(CancelHandle::new()),
            active_model: Mutex::new(None),
            generating: AtomicBool::new(false),
            sd_context: Mutex::new(None),
            model_dir: Mutex::new(model_dir.to_string_lossy().into_owned()),
            upscaler: Mutex::new(None),
        })
    }

    pub fn load_model(&self, paths: ModelPaths, perf: Option<PerfSettings>) -> Result<(), sd_wrapper::SdError> {
        let perf = perf.unwrap_or_default();
        let config = ContextConfig {
            model_path: paths.model_path,
            vae_path: paths.vae_path,
            clip_l_path: paths.clip_l_path,
            t5xxl_path: paths.t5xxl_path,
            diffusion_model_path: paths.diffusion_model_path,
            llm_path: paths.llm_path,
            n_threads: num_cpus(),
            auto_fit: !perf.low_memory,
            low_memory: perf.low_memory,
            max_vram: None,
            control_net_path: paths.control_net_path,
            taesd_path: paths.taesd_path,
            lora_apply_mode: LoraApplyMode::Auto,
        };
        // Loading a model reads several GB off disk and used to be a completely
        // dead UI — no event reached the frontend until the context existed.
        // Forward sd.cpp's load progress as `model:load_progress` so switching
        // models shows movement. The counts are tensors, which is an engine
        // internal: the UI renders a percentage from them and never the number.
        let load_handle = self.app_handle.clone();
        let load_progress_cb: sd_wrapper::progress::ProgressCallback =
            Box::new(move |update: sd_wrapper::ProgressUpdate| {
                let _ = load_handle.emit(
                    "model:load_progress",
                    ModelLoadProgressEvent {
                        step: update.step,
                        total_steps: update.total_steps,
                    },
                );
            });

        // Share cancel_flag and cancel_handle so cancel_generation doesn't need to
        // lock sd_context. The handle is re-pointed at the new sd.cpp context by
        // SdCppContext::new; the OLD context is dropped afterwards (below), so its
        // Drop runs last. That is why the handle's clear() is a compare-and-clear
        // against the dropping context's own pointer — an unconditional clear here
        // would disarm native cancel for every generation after the first switch.
        let ctx = SdContext::with_cancel_flag(
            config,
            self.cancel_flag.clone(),
            self.cancel_handle.clone(),
            Some(load_progress_cb),
        )?;
        let mut lock = self.sd_context.lock().map_err(|e| sd_wrapper::SdError::ContextCreationFailed {
            reason: format!("Lock poisoned: {}", e),
        })?;
        *lock = Some(ctx);
        Ok(())
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `settings.json` written before parameter placement moved to sd.cpp's
    /// auto_fit carries the seven old keys and none of the new one. It must
    /// still *deserialize* — `models.rs` calls `.ok().unwrap_or_default()`, so a
    /// failure here would be silently swallowed and look identical to a user who
    /// has never opened Settings.
    #[test]
    fn perf_settings_deserializes_legacy_json() {
        let legacy = serde_json::json!({
            "flash_attn": true,
            "diffusion_flash_attn": true,
            "enable_mmap": true,
            "free_params_immediately": false,
            "keep_clip_on_cpu": false,
            "keep_vae_on_cpu": false,
            "offload_params_to_cpu": true,
        });
        let parsed = serde_json::from_value::<PerfSettings>(legacy);
        let settings = parsed.expect("legacy perf settings must deserialize, not fall back to Default");
        assert!(!settings.low_memory);
    }
}
