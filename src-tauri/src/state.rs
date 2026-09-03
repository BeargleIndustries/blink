use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use sd_wrapper::{SdContext, ContextConfig, UpscalerContext, LoraApplyMode, CancelHandle};
use serde::{Deserialize, Serialize};

/// The user's only remaining placement choice. When this is off, Blink still
/// forces low-memory placement itself if the model will not fit the GPU
/// (see `low_memory_for`); otherwise sd.cpp's auto_fit plans placement.
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

        // Decide placement per load, never persisted. auto_fit is the fast path
        // only while the main model fits on the GPU; below that, sd.cpp's plan
        // moves the diffusion *compute* to the CPU and a 4 s image takes 4.5 min
        // (measured at 8/6/4/3 GiB budgets — research doc 4b). low_memory keeps
        // compute on the GPU and only parks the weights in RAM: 8 s at the same
        // budgets. So we make that call here instead of trusting the plan.
        let primary_mb = primary_model_mb(&paths);
        let (_, _, free_mb) = crate::commands::system::detect_gpu_info();
        let (low_memory, reason) = low_memory_for(perf.low_memory, free_mb, primary_mb);
        log::info!(
            "placement: {} ({})",
            if low_memory { "low_memory" } else { "auto_fit" },
            reason
        );

        let config = ContextConfig {
            model_path: paths.model_path,
            vae_path: paths.vae_path,
            clip_l_path: paths.clip_l_path,
            t5xxl_path: paths.t5xxl_path,
            diffusion_model_path: paths.diffusion_model_path,
            llm_path: paths.llm_path,
            n_threads: num_cpus(),
            auto_fit: !low_memory,
            low_memory,
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

/// sd.cpp's auto_fit keeps this much VRAM aside for compute buffers before it
/// will place a model's parameters on the GPU (observed: 6272 MiB of Z-Image
/// weights fit a 9 GiB budget and not an 8 GiB one — 6272 + 2048 = 8320).
const SDCPP_COMPUTE_RESERVE_MB: u64 = 2048;
/// sd.cpp subtracts this from the device's free VRAM before budgeting.
const SDCPP_FREE_MARGIN_MB: u64 = 512;

/// Whether to force weights into CPU RAM (`low_memory`) rather than let
/// sd.cpp's auto_fit plan placement.
///
/// Returns the decision and a one-line reason for the log. The user's explicit
/// choice always wins. When free VRAM cannot be read (no `nvidia-smi`), the
/// safe answer is `low_memory`: it costs ~1 s per image on a card that would
/// have fit, and saves minutes on one that would not.
pub(crate) fn low_memory_for(
    user_low_memory: bool,
    free_vram_mb: Option<u64>,
    primary_model_mb: u64,
) -> (bool, String) {
    if user_low_memory {
        return (true, "user setting".to_string());
    }
    let need_mb = primary_model_mb + SDCPP_COMPUTE_RESERVE_MB;
    match free_vram_mb {
        None => (
            true,
            format!("free VRAM unknown; model needs ~{} MiB on the GPU", need_mb),
        ),
        Some(free) => {
            let usable = free.saturating_sub(SDCPP_FREE_MARGIN_MB);
            if usable >= need_mb {
                (
                    false,
                    format!("{} MiB usable >= {} MiB needed", usable, need_mb),
                )
            } else {
                (
                    true,
                    format!(
                        "auto: {} MiB usable < {} MiB needed; auto_fit would move diffusion compute to CPU",
                        usable, need_mb
                    ),
                )
            }
        }
    }
}

/// Size on disk, in MiB, of the model file that has to fit on the GPU — the
/// diffusion model for split stacks, else the single checkpoint. A quantized
/// GGUF's file size is a close proxy for its resident size. 0 if unreadable.
fn primary_model_mb(paths: &ModelPaths) -> u64 {
    let candidate = paths
        .diffusion_model_path
        .as_deref()
        .or(paths.model_path.as_deref());
    candidate
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len() / (1024 * 1024))
        .unwrap_or(0)
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    const ZIMAGE_Q8_MB: u64 = 6272;

    #[test]
    fn user_choice_always_wins() {
        let (low, reason) = low_memory_for(true, Some(20_000), 1_000);
        assert!(low);
        assert_eq!(reason, "user setting");
    }

    #[test]
    fn twelve_gb_card_fits_zimage_so_auto_fit() {
        // nvidia-smi free on the reference machine while idle.
        let (low, _) = low_memory_for(false, Some(11_069), ZIMAGE_Q8_MB);
        assert!(!low);
    }

    #[test]
    fn nine_gib_budget_fits_but_eight_does_not() {
        // The measured cliff: sd.cpp kept everything on CUDA0 at 9 GiB and moved
        // diffusion compute to CPU at 8 GiB.
        let (low_at_9, _) = low_memory_for(false, Some(9 * 1024), ZIMAGE_Q8_MB);
        let (low_at_8, reason) = low_memory_for(false, Some(8 * 1024), ZIMAGE_Q8_MB);
        assert!(!low_at_9);
        assert!(low_at_8);
        assert!(reason.starts_with("auto:"), "reason was: {reason}");
    }

    #[test]
    fn small_model_on_six_gb_card_stays_auto_fit() {
        // SD 1.5 Q5 (~1.6 GB) on a 6 GB card: 6144 - 512 = 5632 >= 1600 + 2048.
        let (low, _) = low_memory_for(false, Some(6_144), 1_600);
        assert!(!low);
    }

    #[test]
    fn small_model_on_four_gb_card_is_low_memory() {
        // Same model on a 4 GB card: 4096 - 512 = 3584 < 3648. The compute
        // reserve is what tips it; low_memory keeps compute on the GPU and only
        // parks the weights in RAM, so this is the safe side of the line.
        let (low, _) = low_memory_for(false, Some(4_096), 1_600);
        assert!(low);
    }

    #[test]
    fn unknown_vram_is_low_memory() {
        let (low, reason) = low_memory_for(false, None, ZIMAGE_Q8_MB);
        assert!(low);
        assert!(reason.contains("unknown"));
    }

    #[test]
    fn boundary_is_inclusive() {
        // usable == need exactly → fits.
        let need = ZIMAGE_Q8_MB + SDCPP_COMPUTE_RESERVE_MB;
        let (low, _) = low_memory_for(false, Some(need + SDCPP_FREE_MARGIN_MB), ZIMAGE_Q8_MB);
        assert!(!low);
        let (low, _) = low_memory_for(false, Some(need + SDCPP_FREE_MARGIN_MB - 1), ZIMAGE_Q8_MB);
        assert!(low);
    }

    #[test]
    fn primary_model_size_prefers_diffusion_model_and_tolerates_missing_files() {
        let paths = ModelPaths {
            model_path: Some("Z:/definitely/not/here.safetensors".into()),
            vae_path: None,
            clip_l_path: None,
            t5xxl_path: None,
            diffusion_model_path: Some("Z:/also/not/here.gguf".into()),
            llm_path: None,
            control_net_path: None,
            taesd_path: None,
        };
        assert_eq!(primary_model_mb(&paths), 0);
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
