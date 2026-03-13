use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use sd_wrapper::{SdContext, ContextConfig};

pub struct AppState {
    pub app_handle: AppHandle,
    pub cancel_flag: Arc<AtomicBool>,
    pub active_model: Mutex<Option<String>>,
    pub generating: AtomicBool,
    pub sd_context: Mutex<Option<SdContext>>,
    pub model_dir: Mutex<String>,
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

    pub fn load_model(&self, model_path: &str) -> Result<(), sd_wrapper::SdError> {
        let config = ContextConfig {
            model_path: model_path.to_string(),
            vae_path: None,
            n_threads: num_cpus(),
        };
        // Share cancel_flag so cancel_generation doesn't need to lock sd_context
        let ctx = SdContext::with_cancel_flag(config, self.cancel_flag.clone())?;
        let mut lock = self.sd_context.lock().unwrap();
        *lock = Some(ctx);
        Ok(())
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
}
