use serde::{Deserialize, Serialize};
use tauri::{State, Emitter};
use tauri_plugin_store::StoreExt;
use crate::state::{AppState, ModelPaths, PerfSettings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

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
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub vram_mb: u64,
    pub architecture: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_512")]
    pub default_width: u32,
    #[serde(default = "default_512")]
    pub default_height: u32,
    #[serde(default = "default_20")]
    pub default_steps: u32,
    #[serde(default = "default_cfg_7")]
    pub default_cfg: f32,
    #[serde(default = "default_sampler")]
    pub default_sampler: String,
    #[serde(default)]
    pub license: Option<ModelLicense>,
    #[serde(default)]
    pub tier: u32,
    #[serde(default)]
    pub files: Option<Vec<ModelFile>>,
    #[serde(default = "default_true")]
    pub available: bool,
    #[serde(default)]
    pub recommended_for: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelFile {
    pub role: String,
    pub repo: String,
    pub filename: String,
    pub size_bytes: u64,
    pub required: bool,
}

fn default_true() -> bool { true }
fn default_512() -> u32 { 512 }
fn default_20() -> u32 { 20 }
fn default_cfg_7() -> f32 { 7.0 }
fn default_sampler() -> String { "euler_a".to_string() }

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
    #[serde(default)]
    files: Option<HashMap<String, FileStatus>>,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileStatus {
    pub status: String,
    pub path: String,
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

    let mut models: Vec<ModelInfo> = manifest.models.iter().filter(|m| m.available).map(|m| {
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
            license_name: m.license.as_ref().map(|l| l.name.clone()).unwrap_or_default(),
            license_url: m.license.as_ref().map(|l| l.url.clone()).unwrap_or_default(),
            commercial: m.license.as_ref().map(|l| l.commercial).unwrap_or(false),
            downloaded,
            active: is_active,
            default_width: m.default_width,
            default_height: m.default_height,
            default_steps: m.default_steps,
            default_cfg: m.default_cfg,
            default_sampler: m.default_sampler.clone(),
        }
    }).collect();

    // Append custom models (metadata entries whose ID starts with "custom-")
    for (id, status) in &metadata.models {
        if id.starts_with("custom-") && status.status == "ready" {
            let full_path = PathBuf::from(&model_dir).join(&status.path);
            let size_bytes = std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(status.size_bytes);
            let display_name = status.name.clone()
                .unwrap_or_else(|| {
                    full_path.file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| id.clone())
                });
            let is_active = active.as_ref() == Some(id);
            let arch = status.architecture.clone().unwrap_or_else(|| "custom".to_string());
            let (def_w, def_h, def_steps, def_cfg, def_sampler) = architecture_defaults(&arch);
            models.push(ModelInfo {
                id: id.clone(),
                name: display_name,
                description: "Custom imported model".to_string(),
                architecture: arch,
                size_bytes,
                vram_mb: 0,
                license_name: "Unknown".to_string(),
                license_url: String::new(),
                commercial: false,
                downloaded: true,
                active: is_active,
                default_width: def_w,
                default_height: def_h,
                default_steps: def_steps,
                default_cfg: def_cfg,
                default_sampler: def_sampler,
            });
        }
    }

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

    // Read HF token from store for gated model access
    let hf_token: Option<String> = state.app_handle.store("settings.json")
        .ok()
        .and_then(|store| store.get("hf_token"))
        .and_then(|val| serde_json::from_value(val).ok())
        .flatten();

    std::thread::spawn(move || {
        // Catch panics so they don't silently kill the thread
        let app_handle_panic = app_handle.clone();
        let model_id_panic = model.id.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let target_dir = PathBuf::from(&model_dir).join(&model.architecture);
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            log::error!("Failed to create model dir: {}", e);
            let _ = app_handle.emit("model:download_error",
                format!("{}: failed to create directory: {}", model.id, e));
            return;
        }

        let api_builder = hf_hub::api::sync::ApiBuilder::new();
        let api = if let Some(ref token) = hf_token {
            api_builder.with_token(Some(token.clone())).build()
        } else {
            api_builder.build()
        };
        let api = match api {
            Ok(api) => api,
            Err(e) => {
                log::error!("Failed to initialize HF API: {}", e);
                let _ = app_handle.emit("model:download_error",
                    format!("{}: HF API init failed: {}", model.id, e));
                return;
            }
        };

        if let Some(ref files) = model.files {
            // Multi-file download
            let total_files = files.len();
            let mut file_statuses: HashMap<String, FileStatus> = HashMap::new();
            let mut metadata = load_metadata(&model_dir);

            // Load existing file statuses if resuming a partial download
            if let Some(existing) = metadata.models.get(&model.id) {
                if let Some(ref existing_files) = existing.files {
                    file_statuses = existing_files.clone();
                }
            }

            log::info!("Starting multi-file download: {} ({} files)", model.name, total_files);

            let mut all_required_ok = true;

            for (idx, file) in files.iter().enumerate() {
                // Skip already-downloaded files
                if let Some(fs) = file_statuses.get(&file.role) {
                    if fs.status == "ready" {
                        let existing_path = PathBuf::from(&model_dir).join(&fs.path);
                        if existing_path.exists() {
                            log::info!("Skipping already-downloaded file: {} ({})", file.filename, file.role);
                            continue;
                        }
                    }
                }

                let _ = app_handle.emit("model:download_file_start", serde_json::json!({
                    "model_id": model.id,
                    "file_role": file.role,
                    "file_index": idx,
                    "total_files": total_files,
                }));

                log::info!("Downloading file {}/{}: {} from {}/{}", idx + 1, total_files, file.role, file.repo, file.filename);

                let repo = api.model(file.repo.clone());
                match repo.get(&file.filename) {
                    Ok(cached_path) => {
                        // Use just the basename for local storage (filename may contain subdirs like "split_files/vae/ae.safetensors")
                        let local_name = std::path::Path::new(&file.filename)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| file.filename.clone());
                        let target_path = target_dir.join(&local_name);
                        if let Err(e) = std::fs::copy(&cached_path, &target_path) {
                            log::error!("Failed to copy file {} to target dir: {}", local_name, e);
                            file_statuses.insert(file.role.clone(), FileStatus {
                                status: "failed".into(),
                                path: format!("{}/{}", model.architecture, local_name),
                            });
                            if file.required {
                                all_required_ok = false;
                            }
                            let _ = app_handle.emit("model:download_error",
                                format!("{}: copy failed for {}: {}", model.id, file.role, e));
                            // Save partial progress
                            metadata.models.insert(model.id.clone(), ModelStatus {
                                status: "partial".into(),
                                path: format!("{}/", model.architecture),
                                downloaded_at: Some(unix_timestamp()),
                                size_bytes: model.size_bytes,
                                sha256_verified: false,
                                files: Some(file_statuses.clone()),
                                architecture: None,
                                name: None,
                            });
                            let _ = save_metadata(&model_dir, &metadata);
                            continue;
                        }

                        file_statuses.insert(file.role.clone(), FileStatus {
                            status: "ready".into(),
                            path: format!("{}/{}", model.architecture, local_name),
                        });

                        let _ = app_handle.emit("model:download_file_complete", serde_json::json!({
                            "model_id": model.id,
                            "file_role": file.role,
                        }));
                    }
                    Err(e) => {
                        let e_str = format!("{}", e);
                        let error_msg = if e_str.contains("401") || e_str.contains("403") || e_str.contains("gated") {
                            format!("{}: {} needs authentication — set your HuggingFace token in Settings", model.id, file.repo)
                        } else {
                            format!("{}: download failed for {}: {}", model.id, file.role, e)
                        };
                        let local_name = std::path::Path::new(&file.filename)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| file.filename.clone());
                        log::error!("Download failed for file {}: {}", local_name, e);
                        file_statuses.insert(file.role.clone(), FileStatus {
                            status: "failed".into(),
                            path: format!("{}/{}", model.architecture, local_name),
                        });
                        if file.required {
                            all_required_ok = false;
                        }
                        let _ = app_handle.emit("model:download_error", error_msg);
                        // Save partial progress
                        metadata.models.insert(model.id.clone(), ModelStatus {
                            status: "partial".into(),
                            path: format!("{}/", model.architecture),
                            downloaded_at: Some(unix_timestamp()),
                            size_bytes: model.size_bytes,
                            sha256_verified: false,
                            files: Some(file_statuses.clone()),
                            architecture: None,
                            name: None,
                        });
                        let _ = save_metadata(&model_dir, &metadata);
                        continue;
                    }
                }
            }

            // Final status
            let final_status = if all_required_ok { "ready" } else { "partial" };
            metadata.models.insert(model.id.clone(), ModelStatus {
                status: final_status.into(),
                path: format!("{}/", model.architecture),
                downloaded_at: Some(unix_timestamp()),
                size_bytes: model.size_bytes,
                sha256_verified: false,
                files: Some(file_statuses),
                architecture: None,
                name: None,
            });
            let _ = save_metadata(&model_dir, &metadata);

            if all_required_ok {
                let _ = app_handle.emit("model:download_complete", &model.id);
                log::info!("Multi-file download complete: {}", model.name);
            } else {
                let _ = app_handle.emit("model:download_error",
                    format!("{}: some required files failed to download", model.id));
                log::error!("Multi-file download incomplete: {}", model.name);
            }
        } else {
            // Single-file download
            let repo_name = match model.repo.as_ref() {
                Some(r) => r,
                None => {
                    let _ = app_handle.emit("model:download_error",
                        format!("{}: no repo specified", model.id));
                    return;
                }
            };
            let file_name = match model.filename.as_ref() {
                Some(f) => f,
                None => {
                    let _ = app_handle.emit("model:download_error",
                        format!("{}: no filename specified", model.id));
                    return;
                }
            };

            log::info!("Starting download: {} from {}/{}", model.name, repo_name, file_name);

            let target_path = target_dir.join(file_name);

            let repo = api.model(repo_name.clone());
            match repo.get(file_name) {
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
                        path: format!("{}/{}", model.architecture, file_name),
                        downloaded_at: Some(unix_timestamp()),
                        size_bytes: model.size_bytes,
                        sha256_verified: false,
                        files: None,
                        architecture: None,
                        name: None,
                    });
                    let _ = save_metadata(&model_dir, &metadata);

                    let _ = app_handle.emit("model:download_complete", &model.id);
                    log::info!("Download complete: {}", model.name);
                }
                Err(e) => {
                    let e_str = format!("{}", e);
                    let error_msg = if e_str.contains("401") || e_str.contains("403") || e_str.contains("gated") {
                        format!("{}: {} needs authentication — set your HuggingFace token in Settings", model.id, repo_name)
                    } else {
                        format!("{}: {}", model.id, e)
                    };
                    log::error!("Download failed: {}", e);
                    let _ = app_handle.emit("model:download_error", error_msg);
                }
            }
        }
        })); // end catch_unwind
        if let Err(panic) = result {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic in download thread".to_string()
            };
            log::error!("Download thread panicked: {}", msg);
            let _ = app_handle_panic.emit("model:download_error",
                format!("{}: internal error: {}", model_id_panic, msg));
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
        if let Some(ref files) = model_status.files {
            // Multi-file model: delete each file individually
            for file_status in files.values() {
                let file_path = PathBuf::from(&model_dir).join(&file_status.path);
                if file_path.exists() {
                    let _ = std::fs::remove_file(&file_path);
                }
            }
            // For custom multi-file models, also clean up the model subdirectory
            let model_path = PathBuf::from(&model_dir).join(&model_status.path);
            if model_path.is_dir() {
                let _ = std::fs::remove_dir(&model_path);
            }
            // Try to remove the architecture directory if now empty
            if let Some(parent) = PathBuf::from(&model_dir).join(&model_status.path).parent() {
                let _ = std::fs::remove_dir(parent); // only succeeds if empty
            }
        } else {
            // Single-file model
            let full_path = PathBuf::from(&model_dir).join(&model_status.path);
            if full_path.exists() {
                std::fs::remove_file(&full_path).map_err(|e| e.to_string())?;
            }
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

    // Load perf settings from store (fall back to defaults)
    let mut perf = state.app_handle.store("settings.json")
        .ok()
        .and_then(|store| store.get("perf_settings"))
        .and_then(|val| serde_json::from_value::<PerfSettings>(val).ok())
        .unwrap_or_default();

    // Custom models: single-file or multi-file, no manifest entry needed
    if model_id.starts_with("custom-") {
        if let Some(ref file_statuses) = model_status.files {
            // Multi-file custom model — resolve paths like manifest models
            let model_subdir = PathBuf::from(&model_dir).join(&model_status.path);
            let path_for_role = |role: &str| -> Option<String> {
                file_statuses.get(role).map(|fs| {
                    let full = model_subdir.join(&fs.path);
                    full.to_string_lossy().into_owned()
                })
            };

            let paths = ModelPaths {
                model_path: None,
                diffusion_model_path: path_for_role("diffusion_model"),
                clip_l_path: path_for_role("clip_l"),
                t5xxl_path: path_for_role("t5xxl"),
                vae_path: path_for_role("vae"),
                llm_path: path_for_role("llm"),
                control_net_path: None,
                taesd_path: None,
            };

            // Perf auto-adjustment for multi-file custom models
            if let Some(ref arch) = model_status.architecture {
                match arch.as_str() {
                    "flux" | "flux-kontext" | "z-image" => {
                        perf.offload_params_to_cpu = true;
                        eprintln!("[blink] Auto-enabled offload-to-cpu for custom {} model", arch);
                    }
                    _ => {}
                }
            }

            state.load_model(paths, Some(perf)).map_err(|e| e.to_string())?;
            let mut active = state.active_model.lock().map_err(|e| e.to_string())?;
            *active = Some(model_id);
            return Ok(());
        } else {
            // Legacy single-file custom model — existing behavior
            let model_path = PathBuf::from(&model_dir).join(&model_status.path);
            if !model_path.exists() {
                return Err(format!("Model file not found: {}", model_path.display()));
            }
            let paths = ModelPaths {
                model_path: Some(model_path.to_string_lossy().into_owned()),
                vae_path: None,
                clip_l_path: None,
                t5xxl_path: None,
                diffusion_model_path: None,
                llm_path: None,
                control_net_path: None,
                taesd_path: None,
            };

            // Perf auto-adjustment for single-file custom models with known architecture
            if let Some(ref arch) = model_status.architecture {
                match arch.as_str() {
                    "flux" | "flux-kontext" | "z-image" => {
                        perf.offload_params_to_cpu = true;
                        eprintln!("[blink] Auto-enabled offload-to-cpu for custom {} model", arch);
                    }
                    _ => {}
                }
            }

            state.load_model(paths, Some(perf)).map_err(|e| e.to_string())?;
            let mut active = state.active_model.lock().map_err(|e| e.to_string())?;
            *active = Some(model_id);
            return Ok(());
        }
    }

    let manifest = load_manifest()?;
    let manifest_model = manifest.models.iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model '{}' not found in manifest", model_id))?
        .clone();

    let paths = if let Some(ref manifest_files) = manifest_model.files {
        // Multi-file model (e.g. Flux): look up each role's path from metadata
        let file_statuses = model_status.files
            .ok_or_else(|| format!("Model '{}' has no file status in metadata", model_id))?;

        let path_for_role = |role: &str| -> Option<String> {
            file_statuses.get(role).and_then(|fs| {
                let full = PathBuf::from(&model_dir).join(&fs.path);
                if full.exists() { Some(full.to_string_lossy().into_owned()) } else { None }
            })
        };

        // Verify all required files are present
        for f in manifest_files {
            if f.required {
                path_for_role(&f.role).ok_or_else(|| {
                    format!("Required file '{}' for model '{}' not found", f.role, model_id)
                })?;
            }
        }

        ModelPaths {
            model_path: None,
            diffusion_model_path: path_for_role("diffusion_model"),
            clip_l_path: path_for_role("clip_l"),
            t5xxl_path: path_for_role("t5xxl"),
            vae_path: path_for_role("vae"),
            llm_path: path_for_role("llm"),
            control_net_path: None,
            taesd_path: None,
        }
    } else {
        // Single-file model (SD1.5, SDXL, etc.)
        let model_path = PathBuf::from(&model_dir).join(&model_status.path);
        if !model_path.exists() {
            return Err(format!("Model file not found: {}", model_path.display()));
        }
        ModelPaths {
            model_path: Some(model_path.to_string_lossy().into_owned()),
            vae_path: None,
            clip_l_path: None,
            t5xxl_path: None,
            diffusion_model_path: None,
            llm_path: None,
            control_net_path: None,
            taesd_path: None,
        }
    };

    // Auto-adjust perf settings based on model architecture.
    // Reset architecture-specific overrides first so switching models doesn't carry over stale settings.
    // (keep_vae_on_cpu produces black output — the CPU backend can't decode GPU latents correctly)
    perf.offload_params_to_cpu = false;
    perf.keep_clip_on_cpu = false;
    perf.keep_vae_on_cpu = false;
    perf.free_params_immediately = false;

    let arch = &manifest_model.architecture;
    match arch.as_str() {
        "flux" | "flux-kontext" | "z-image" => {
            // Large models that may exceed VRAM. Enable offload to swap weights
            // through CPU RAM while keeping compute on GPU.
            perf.offload_params_to_cpu = true;
            eprintln!("[blink] Auto-enabled offload-to-cpu for {} architecture", arch);
        }
        // SD 1.5, SDXL, custom: defaults are fine (all GPU, no offload)
        _ => {}
    }

    state.load_model(paths, Some(perf))
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

#[tauri::command]
pub async fn import_custom_model(
    state: State<'_, AppState>,
    url: String,
    name: Option<String>,
) -> Result<(), String> {
    let (repo, filename) = parse_hf_url(&url)?;

    if !filename.ends_with(".gguf") && !filename.ends_with(".safetensors") {
        return Err("Only .gguf and .safetensors files are supported".to_string());
    }

    // Sanitize filename — prevent path traversal
    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .ok_or_else(|| "Invalid filename".to_string())?
        .to_string_lossy()
        .to_string();

    let model_id = format!("custom-{}", {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    });

    // Derive display name: use provided name, or strip extension from filename
    let display_name = name.unwrap_or_else(|| {
        safe_filename
            .strip_suffix(".gguf")
            .or_else(|| safe_filename.strip_suffix(".safetensors"))
            .unwrap_or(&safe_filename)
            .to_string()
    });

    let architecture = detect_architecture(&filename, &repo);
    let companions = get_companion_files(&architecture);

    let model_dir = state.model_dir.lock().map_err(|e| e.to_string())?.clone();
    let app_handle = state.app_handle.clone();

    // Read HF token from store for gated model access
    let hf_token: Option<String> = state.app_handle.store("settings.json")
        .ok()
        .and_then(|store| store.get("hf_token"))
        .and_then(|val| serde_json::from_value(val).ok())
        .flatten();

    std::thread::spawn(move || {
        let api_builder = hf_hub::api::sync::ApiBuilder::new();
        let api = if let Some(ref token) = hf_token {
            api_builder.with_token(Some(token.clone())).build()
        } else {
            api_builder.build()
        };
        let api = match api {
            Ok(a) => a,
            Err(e) => {
                log::error!("Failed to initialize HF API: {}", e);
                let _ = app_handle.emit("model:download_error",
                    format!("{}: HF API init failed: {}", model_id, e));
                return;
            }
        };

        if !companions.is_empty() {
            // Multi-file download: create subdirectory
            let target_dir = PathBuf::from(&model_dir).join("custom").join(&model_id);
            if let Err(e) = std::fs::create_dir_all(&target_dir) {
                log::error!("Failed to create custom model dir: {}", e);
                let _ = app_handle.emit("model:download_error",
                    format!("{}: failed to create directory: {}", model_id, e));
                return;
            }

            let total_files = 1 + companions.len();
            let mut file_statuses: HashMap<String, FileStatus> = HashMap::new();

            // Download primary file as "diffusion_model" role
            let _ = app_handle.emit("model:download_file_start",
                format!("{}: downloading diffusion_model (1/{})", model_id, total_files));

            log::info!("Importing custom model: {} from {}/{}", display_name, repo, filename);
            let hf_repo = api.model(repo.clone());
            match hf_repo.get(&filename) {
                Ok(cached_path) => {
                    let target_path = target_dir.join(&safe_filename);
                    if let Err(e) = std::fs::copy(&cached_path, &target_path) {
                        log::error!("Failed to copy custom model: {}", e);
                        let _ = app_handle.emit("model:download_error",
                            format!("{}: copy failed: {}", model_id, e));
                        return;
                    }
                    file_statuses.insert("diffusion_model".into(), FileStatus {
                        path: safe_filename.clone(),
                        status: "ready".into(),
                    });
                    let _ = app_handle.emit("model:download_file_complete", serde_json::json!({
                        "model_id": model_id,
                        "file_role": "diffusion_model",
                    }));
                }
                Err(e) => {
                    log::error!("Custom model download failed: {}", e);
                    let _ = app_handle.emit("model:download_error",
                        format!("{}: {}", model_id, e));
                    return;
                }
            }

            // Download companion files
            for (i, companion) in companions.iter().enumerate() {
                let file_index = i + 2; // 1-indexed, primary was 1
                let _ = app_handle.emit("model:download_file_start",
                    format!("{}: downloading {} ({}/{})", model_id, companion.role, file_index, total_files));

                let companion_repo = api.model(companion.repo.clone());
                match companion_repo.get(&companion.filename) {
                    Ok(cached_path) => {
                        let target_filename = Path::new(&companion.filename)
                            .file_name()
                            .unwrap_or(OsStr::new(&companion.filename))
                            .to_string_lossy().to_string();
                        let target = target_dir.join(&target_filename);
                        if let Err(e) = std::fs::copy(&cached_path, &target) {
                            log::error!("Failed to copy companion file {}: {}", companion.role, e);
                            let _ = app_handle.emit("model:download_error",
                                format!("{}: copy failed for {}: {}", model_id, companion.role, e));
                            return;
                        }
                        file_statuses.insert(companion.role.clone(), FileStatus {
                            path: target_filename,
                            status: "ready".into(),
                        });
                        let _ = app_handle.emit("model:download_file_complete", serde_json::json!({
                            "model_id": model_id,
                            "file_role": companion.role,
                        }));
                    }
                    Err(e) => {
                        log::error!("Companion download failed for {}: {}", companion.role, e);
                        let _ = app_handle.emit("model:download_error",
                            format!("{}: download failed for {}: {}", model_id, companion.role, e));
                        return;
                    }
                }
            }

            // All files downloaded — save metadata
            let total_size: u64 = file_statuses.values()
                .filter_map(|fs| {
                    let full_path = target_dir.join(&fs.path);
                    std::fs::metadata(&full_path).ok().map(|m| m.len())
                })
                .sum();

            let mut metadata = load_metadata(&model_dir);
            metadata.models.insert(model_id.clone(), ModelStatus {
                status: "ready".into(),
                path: format!("custom/{}/", model_id),
                downloaded_at: Some(unix_timestamp()),
                size_bytes: total_size,
                sha256_verified: false,
                files: Some(file_statuses),
                architecture: Some(architecture),
                name: Some(display_name.clone()),
            });
            if let Err(e) = save_metadata(&model_dir, &metadata) {
                log::error!("Failed to save metadata for custom model: {}", e);
            }

            let _ = app_handle.emit("model:download_complete", &model_id);
            log::info!("Custom model import complete (multi-file): {}", display_name);
        } else {
            // Single-file download (sd1, sdxl): flat custom/ directory
            let target_dir = PathBuf::from(&model_dir).join("custom");
            if let Err(e) = std::fs::create_dir_all(&target_dir) {
                log::error!("Failed to create custom model dir: {}", e);
                let _ = app_handle.emit("model:download_error",
                    format!("{}: failed to create directory: {}", model_id, e));
                return;
            }

            log::info!("Importing custom model: {} from {}/{}", display_name, repo, filename);
            let hf_repo = api.model(repo.clone());
            match hf_repo.get(&filename) {
                Ok(cached_path) => {
                    let target_path = target_dir.join(&safe_filename);
                    if let Err(e) = std::fs::copy(&cached_path, &target_path) {
                        log::error!("Failed to copy custom model: {}", e);
                        let _ = app_handle.emit("model:download_error",
                            format!("{}: copy failed: {}", model_id, e));
                        return;
                    }

                    let size_bytes = std::fs::metadata(&target_path).map(|m| m.len()).unwrap_or(0);

                    let mut metadata = load_metadata(&model_dir);
                    metadata.models.insert(model_id.clone(), ModelStatus {
                        status: "ready".into(),
                        path: format!("custom/{}", safe_filename),
                        downloaded_at: Some(unix_timestamp()),
                        size_bytes,
                        sha256_verified: false,
                        files: None,
                        architecture: Some(architecture),
                        name: Some(display_name.clone()),
                    });
                    if let Err(e) = save_metadata(&model_dir, &metadata) {
                        log::error!("Failed to save metadata for custom model: {}", e);
                    }

                    let _ = app_handle.emit("model:download_complete", &model_id);
                    log::info!("Custom model import complete: {}", display_name);
                }
                Err(e) => {
                    log::error!("Custom model download failed: {}", e);
                    let _ = app_handle.emit("model:download_error",
                        format!("{}: {}", model_id, e));
                }
            }
        }
    });

    Ok(())
}

fn parse_hf_url(url: &str) -> Result<(String, String), String> {
    // https://huggingface.co/{owner}/{repo}/blob/main/{filename}
    // https://huggingface.co/{owner}/{repo}/resolve/main/{filename}
    if url.starts_with("https://huggingface.co/") {
        let path = url.trim_start_matches("https://huggingface.co/");
        let parts: Vec<&str> = path.splitn(5, '/').collect();
        if parts.len() >= 5 && (parts[2] == "blob" || parts[2] == "resolve") {
            let repo = format!("{}/{}", parts[0], parts[1]);
            let filename = parts[4..].join("/");
            return Ok((repo, filename));
        }
        return Err(format!("Could not parse HuggingFace URL: {}", url));
    }

    // owner/repo:filename
    if let Some(colon_pos) = url.find(':') {
        let repo = url[..colon_pos].to_string();
        let filename = url[colon_pos + 1..].to_string();
        if !repo.is_empty() && !filename.is_empty() {
            return Ok((repo, filename));
        }
    }

    Err("Unrecognized URL format. Use:\n  https://huggingface.co/owner/repo/blob/main/model.gguf\n  owner/repo:model.gguf".to_string())
}

#[derive(Debug, Clone)]
struct CompanionFile {
    role: String,
    repo: String,
    filename: String,
    size_bytes: u64,
}

fn detect_architecture(filename: &str, repo: &str) -> String {
    let fname_lower = filename.to_lowercase();
    let repo_lower = repo.to_lowercase();

    // Order matters — check more specific patterns first
    if fname_lower.contains("z-image") || fname_lower.contains("z_image")
        || repo_lower.contains("z-image") || repo_lower.contains("z_image") {
        "z-image".to_string()
    } else if fname_lower.contains("kontext") || repo_lower.contains("kontext") {
        "flux-kontext".to_string()
    } else if fname_lower.contains("flux") || repo_lower.contains("flux") {
        "flux".to_string()
    } else if fname_lower.contains("wan") || repo_lower.contains("wan") {
        "wan".to_string()
    } else if fname_lower.contains("sdxl") || fname_lower.contains("sd_xl")
        || repo_lower.contains("sdxl") || repo_lower.contains("sd_xl") {
        "sdxl".to_string()
    } else if fname_lower.contains("sd3") || repo_lower.contains("sd3") {
        "sd3".to_string()
    } else {
        "sd1".to_string()
    }
}

fn get_companion_files(architecture: &str) -> Vec<CompanionFile> {
    match architecture {
        "flux" | "flux-kontext" => vec![
            CompanionFile { role: "clip_l".into(), repo: "comfyanonymous/flux_text_encoders".into(), filename: "clip_l.safetensors".into(), size_bytes: 300_000_000 },
            CompanionFile { role: "t5xxl".into(), repo: "city96/t5-v1_1-xxl-encoder-gguf".into(), filename: "t5-v1_1-xxl-encoder-Q4_K_M.gguf".into(), size_bytes: 2_500_000_000 },
            CompanionFile { role: "vae".into(), repo: "Comfy-Org/z_image_turbo".into(), filename: "split_files/vae/ae.safetensors".into(), size_bytes: 200_000_000 },
        ],
        "z-image" => vec![
            CompanionFile { role: "llm".into(), repo: "unsloth/Qwen3-4B-Instruct-2507-GGUF".into(), filename: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf".into(), size_bytes: 2_800_000_000 },
            CompanionFile { role: "vae".into(), repo: "Comfy-Org/z_image_turbo".into(), filename: "split_files/vae/ae.safetensors".into(), size_bytes: 200_000_000 },
        ],
        "wan" => vec![
            CompanionFile { role: "clip_l".into(), repo: "comfyanonymous/flux_text_encoders".into(), filename: "clip_l.safetensors".into(), size_bytes: 300_000_000 },
            CompanionFile { role: "t5xxl".into(), repo: "city96/t5-v1_1-xxl-encoder-gguf".into(), filename: "t5-v1_1-xxl-encoder-Q4_K_M.gguf".into(), size_bytes: 2_500_000_000 },
            CompanionFile { role: "vae".into(), repo: "Wan-AI/Wan2.1-T2V-1.3B".into(), filename: "Wan2.1_VAE.pth".into(), size_bytes: 400_000_000 },
        ],
        _ => vec![], // sd1, sdxl, sd3 — single file, no companions
    }
}

#[derive(serde::Serialize)]
pub struct LoraFileInfo {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn scan_lora_directory(path: String) -> Result<Vec<LoraFileInfo>, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if ext == "safetensors" || ext == "gguf" {
                    if let Ok(meta) = entry.metadata() {
                        results.push(LoraFileInfo {
                            path: path.to_string_lossy().to_string(),
                            filename: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            size_bytes: meta.len(),
                        });
                    }
                }
            }
        }
    }
    Ok(results)
}

fn architecture_defaults(arch: &str) -> (u32, u32, u32, f32, String) {
    match arch {
        "flux" => (1024, 1024, 4, 1.0, "euler".into()),
        "flux-kontext" => (1024, 1024, 25, 3.5, "euler".into()),
        "z-image" => (512, 1024, 4, 1.0, "euler".into()),
        "wan" => (832, 480, 30, 6.0, "euler".into()),
        "sdxl" => (1024, 1024, 25, 7.0, "euler_a".into()),
        _ => (512, 512, 20, 7.0, "euler_a".into()), // sd1 default
    }
}

fn unix_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}
