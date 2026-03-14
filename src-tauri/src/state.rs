use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use sd_wrapper::{SdContext, ContextConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSettings {
    pub flash_attn: bool,
    pub diffusion_flash_attn: bool,
    pub enable_mmap: bool,
    pub free_params_immediately: bool,
    pub keep_clip_on_cpu: bool,
    pub keep_vae_on_cpu: bool,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
}

impl Default for PerfSettings {
    fn default() -> Self {
        Self {
            flash_attn: true,
            diffusion_flash_attn: true,
            enable_mmap: true,
            free_params_immediately: false,
            keep_clip_on_cpu: false,
            keep_vae_on_cpu: false,
            offload_params_to_cpu: false,
        }
    }
}

pub struct AppState {
    pub app_handle: AppHandle,
    pub cancel_flag: Arc<AtomicBool>,
    pub active_model: Mutex<Option<String>>,
    pub generating: AtomicBool,
    pub sd_context: Mutex<Option<SdContext>>,
    pub model_dir: Mutex<String>,
}

pub struct ModelPaths {
    pub model_path: Option<String>,
    pub vae_path: Option<String>,
    pub clip_l_path: Option<String>,
    pub t5xxl_path: Option<String>,
    pub diffusion_model_path: Option<String>,
    pub llm_path: Option<String>,
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
            active_model: Mutex::new(None),
            generating: AtomicBool::new(false),
            sd_context: Mutex::new(None),
            model_dir: Mutex::new(model_dir.to_string_lossy().into_owned()),
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
            flash_attn: perf.flash_attn,
            diffusion_flash_attn: perf.diffusion_flash_attn,
            enable_mmap: perf.enable_mmap,
            free_params_immediately: perf.free_params_immediately,
            keep_clip_on_cpu: perf.keep_clip_on_cpu,
            keep_vae_on_cpu: perf.keep_vae_on_cpu,
            offload_params_to_cpu: perf.offload_params_to_cpu,
        };
        // Share cancel_flag so cancel_generation doesn't need to lock sd_context
        let ctx = SdContext::with_cancel_flag(config, self.cancel_flag.clone())?;
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
