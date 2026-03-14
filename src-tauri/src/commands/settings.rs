use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_store::StoreExt;
use crate::state::{AppState, PerfSettings};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub model_directory: Option<String>,
    pub default_save_directory: Option<String>,
    pub show_advanced: bool,
    pub last_model: Option<String>,
    pub gallery_columns: u32,
    pub auto_save_gallery: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            model_directory: None,
            default_save_directory: None,
            show_advanced: false,
            last_model: None,
            gallery_columns: 4,
            auto_save_gallery: true,
        }
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    match store.get("settings") {
        Some(val) => serde_json::from_value(val).map_err(|e| e.to_string()),
        None => Ok(AppSettings::default()),
    }
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    store.set("settings", serde_json::to_value(&settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;
    log::info!("Settings saved: theme={}", settings.theme);
    Ok(())
}

#[tauri::command]
pub async fn get_perf_settings(state: State<'_, AppState>) -> Result<PerfSettings, String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    match store.get("perf_settings") {
        Some(val) => serde_json::from_value(val).map_err(|e| e.to_string()),
        None => Ok(PerfSettings::default()),
    }
}

#[tauri::command]
pub async fn save_perf_settings(state: State<'_, AppState>, settings: PerfSettings) -> Result<(), String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    store.set("perf_settings", serde_json::to_value(&settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;
    log::info!("Perf settings saved: flash_attn={}, mmap={}", settings.flash_attn, settings.enable_mmap);
    Ok(())
}

#[tauri::command]
pub async fn get_hf_token(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    match store.get("hf_token") {
        Some(val) => serde_json::from_value(val).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn set_hf_token(state: State<'_, AppState>, token: Option<String>) -> Result<(), String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    store.set("hf_token", serde_json::to_value(&token).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_anthropic_key(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    match store.get("anthropic_key") {
        Some(val) => serde_json::from_value(val).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn set_anthropic_key(state: State<'_, AppState>, key: Option<String>) -> Result<(), String> {
    let store = state.app_handle.store("settings.json").map_err(|e| e.to_string())?;
    store.set("anthropic_key", serde_json::to_value(&key).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
