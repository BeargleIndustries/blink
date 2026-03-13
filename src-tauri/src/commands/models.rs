use serde::{Deserialize, Serialize};
use tauri::{State, Manager, Emitter};
use crate::state::AppState;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub models: Vec<ManifestModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub repo: String,
    pub filename: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub vram_mb: u64,
    pub architecture: String,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u32,
    pub default_cfg: f32,
    pub default_sampler: String,
    pub license: ModelLicense,
    pub tier: u32,
    #[serde(default = "default_true")]
    pub available: bool,
    pub recommended_for: Vec<String>,
}

fn default_true() -> bool { true }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelLicense {
    pub name: String,
    pub url: String,
    pub commercial: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub architecture: String,
    pub size_bytes: u64,
    pub vram_mb: u64,
    pub license_name: String,
    pub license_url: String,
    pub commercial: bool,
    pub downloaded: bool,
    pub active: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u32,
    pub default_cfg: f32,
    pub default_sampler: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ModelMetadata {
    models: std::collections::HashMap<String, ModelStatus>,
    active_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ModelStatus {
    status: String,
    path: String,
    downloaded_at: Option<String>,
    size_bytes: u64,
    sha256_verified: bool,
}

fn load_manifest() -> Result<ModelManifest, String> {
    let manifest_str = include_str!("../../../models.json");
    serde_json::from_str(manifest_str).map_err(|e| format!("Failed to parse models.json: {}", e))
}

fn get_metadata_path(model_dir: &str) -> PathBuf {
    PathBuf::from(model_dir).join("metadata.json")
}

fn load_metadata(model_dir: &str) -> ModelMetadata {
    let path = get_metadata_path(model_dir);
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(meta) = serde_json::from_str(&data) {
                return meta;
            }
        }
    }
    ModelMetadata {
        models: std::collections::HashMap::new(),
        active_model: None,
    }
}

fn save_metadata(model_dir: &str, metadata: &ModelMetadata) -> Result<(), String> {
    let path = get_metadata_path(model_dir);
    let data = serde_json::to_string_pretty(metadata).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let manifest = load_manifest()?;
    let model_dir = state.model_dir.lock().map_err(|e| e.to_string())?.clone();
    let metadata = load_metadata(&model_dir);
    let active = state.active_model.lock().map_err(|e| e.to_string())?.clone();

    let models = manifest.models.iter().filter(|m| m.available).map(|m| {
        let downloaded = metadata.models.get(&m.id)
            .map(|s| s.status == "ready")
            .unwrap_or(false);
        let is_active = active.as_ref() == Some(&m.id);

        ModelInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            description: m.description.clone(),
            architecture: m.architecture.clone(),
            size_bytes: m.size_bytes,
            vram_mb: m.vram_mb,
            license_name: m.license.name.clone(),
            license_url: m.license.url.clone(),
            commercial: m.license.commercial,
            downloaded,
            active: is_active,
            default_width: m.default_width,
            default_height: m.default_height,
            default_steps: m.default_steps,
            default_cfg: m.default_cfg,
            default_sampler: m.default_sampler.clone(),
        }
    }).collect();

    Ok(models)
}

#[tauri::command]
pub async fn get_downloaded_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let all = get_models(state).await?;
    Ok(all.into_iter().filter(|m| m.downloaded).collect())
}

#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let manifest = load_manifest()?;
    let model = manifest.models.iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model '{}' not found in manifest", model_id))?
        .clone();

    let model_dir = state.model_dir.lock().map_err(|e| e.to_string())?.clone();
    let app_handle = state.app_handle.clone();

    std::thread::spawn(move || {
        log::info!("Starting download: {} from {}/{}", model.name, model.repo, model.filename);

        let target_dir = PathBuf::from(&model_dir).join(&model.architecture);
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            log::error!("Failed to create model dir: {}", e);
            let _ = app_handle.emit("model:download_error",
                format!("{}: failed to create directory: {}", model.id, e));
            return;
        }

        let target_path = target_dir.join(&model.filename);

        match hf_hub::api::sync::Api::new() {
            Ok(api) => {
                let repo = api.model(model.repo.clone());
                match repo.get(&model.filename) {
                    Ok(cached_path) => {
                        if let Err(e) = std::fs::copy(&cached_path, &target_path) {
                            log::error!("Failed to copy model to target dir: {}", e);
                            let _ = app_handle.emit("model:download_error",
                                format!("{}: copy failed: {}", model.id, e));
                            return;
                        }

                        let mut metadata = load_metadata(&model_dir);
                        metadata.models.insert(model.id.clone(), ModelStatus {
                            status: "ready".into(),
                            path: format!("{}/{}", model.architecture, model.filename),
                            downloaded_at: Some(unix_timestamp()),
                            size_bytes: model.size_bytes,
                            sha256_verified: false,
                        });
                        let _ = save_metadata(&model_dir, &metadata);

                        let _ = app_handle.emit("model:download_complete", &model.id);
                        log::info!("Download complete: {}", model.name);
                    }
                    Err(e) => {
                        log::error!("Download failed: {}", e);
                        let _ = app_handle.emit("model:download_error",
                            format!("{}: {}", model.id, e));
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to initialize HF API: {}", e);
                let _ = app_handle.emit("model:download_error",
                    format!("{}: HF API init failed: {}", model.id, e));
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn delete_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let model_dir = state.model_dir.lock().map_err(|e| e.to_string())?.clone();
    let mut metadata = load_metadata(&model_dir);

    if let Some(model_status) = metadata.models.remove(&model_id) {
        let full_path = PathBuf::from(&model_dir).join(&model_status.path);
        if full_path.exists() {
            std::fs::remove_file(&full_path).map_err(|e| e.to_string())?;
        }
        save_metadata(&model_dir, &metadata)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn set_active_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let model_dir = state.model_dir.lock().map_err(|e| e.to_string())?.clone();
    let metadata = load_metadata(&model_dir);

    let model_status = metadata.models.get(&model_id)
        .ok_or_else(|| format!("Model '{}' not downloaded", model_id))?
        .clone();

    let model_path = PathBuf::from(&model_dir).join(&model_status.path);
    if !model_path.exists() {
        return Err(format!("Model file not found: {}", model_path.display()));
    }

    state.load_model(&model_path.to_string_lossy())
        .map_err(|e| e.to_string())?;

    let mut active = state.active_model.lock().map_err(|e| e.to_string())?;
    *active = Some(model_id);

    Ok(())
}

#[tauri::command]
pub async fn get_download_progress(
    _model_id: String,
) -> Result<Option<DownloadProgress>, String> {
    // TODO: Track actual download progress with shared state
    Ok(None)
}

fn unix_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}
