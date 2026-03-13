use serde::{Deserialize, Serialize};
use tauri::{State, Manager};
use crate::state::AppState;
use std::path::PathBuf;
use base64::Engine;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GalleryItem {
    pub id: String,
    pub filename: String,
    pub thumbnail_path: String,
    pub full_path: String,
    pub prompt: String,
    pub negative_prompt: String,
    pub model_id: String,
    pub model_name: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: i64,
    pub sampler: String,
    pub generated_at: String,
    pub generation_time_secs: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GalleryMetadata {
    pub prompt: String,
    pub negative_prompt: String,
    pub model_id: String,
    pub model_name: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: i64,
    pub sampler: String,
    pub generated_at: String,
    pub generation_time_secs: f32,
}

fn get_gallery_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let gallery = app_data.join("gallery");
    std::fs::create_dir_all(&gallery).map_err(|e| e.to_string())?;
    Ok(gallery)
}

#[tauri::command]
pub async fn get_gallery(state: State<'_, AppState>) -> Result<Vec<GalleryItem>, String> {
    let gallery_dir = get_gallery_dir(&state.app_handle)?;
    let mut items = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&gallery_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(meta) = serde_json::from_str::<GalleryMetadata>(&data) {
                        let stem = path.file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let png_path = gallery_dir.join(format!("{}.png", stem));
                        let thumb_path = gallery_dir.join(format!("{}_thumb.jpg", stem));

                        if png_path.exists() {
                            items.push(GalleryItem {
                                id: stem.clone(),
                                filename: format!("{}.png", stem),
                                thumbnail_path: thumb_path.to_string_lossy().into_owned(),
                                full_path: png_path.to_string_lossy().into_owned(),
                                prompt: meta.prompt,
                                negative_prompt: meta.negative_prompt,
                                model_id: meta.model_id,
                                model_name: meta.model_name,
                                width: meta.width,
                                height: meta.height,
                                steps: meta.steps,
                                cfg_scale: meta.cfg_scale,
                                seed: meta.seed,
                                sampler: meta.sampler,
                                generated_at: meta.generated_at,
                                generation_time_secs: meta.generation_time_secs,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by generated_at descending (newest first)
    items.sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
    Ok(items)
}

#[tauri::command]
pub async fn delete_gallery_item(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<(), String> {
    let gallery_dir = get_gallery_dir(&state.app_handle)?;

    let png = gallery_dir.join(format!("{}.png", item_id));
    let json = gallery_dir.join(format!("{}.json", item_id));
    let thumb = gallery_dir.join(format!("{}_thumb.jpg", item_id));

    if png.exists() { std::fs::remove_file(&png).map_err(|e| e.to_string())?; }
    if json.exists() { std::fs::remove_file(&json).map_err(|e| e.to_string())?; }
    if thumb.exists() { std::fs::remove_file(&thumb).map_err(|e| e.to_string())?; }

    Ok(())
}

#[tauri::command]
pub async fn save_to_gallery(
    state: State<'_, AppState>,
    image_base64: String,
    prompt: String,
    negative_prompt: String,
    model_id: String,
    model_name: String,
    width: u32,
    height: u32,
    steps: u32,
    cfg_scale: f32,
    seed: i64,
    sampler: String,
    generation_time_secs: f32,
) -> Result<GalleryItem, String> {
    let gallery_dir = get_gallery_dir(&state.app_handle)?;

    // Generate unique ID: timestamp + random suffix
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let id = format!("{}_{:06}", now.as_millis(), rand_suffix());

    // Decode base64 PNG
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Save PNG file
    let png_path = gallery_dir.join(format!("{}.png", id));
    std::fs::write(&png_path, &png_bytes).map_err(|e| e.to_string())?;

    // Create thumbnail (resize to 150x150) using image crate
    let thumb_path = gallery_dir.join(format!("{}_thumb.jpg", id));
    {
        let img = image::load_from_memory(&png_bytes)
            .map_err(|e| format!("Failed to load image: {}", e))?;
        let thumb = img.resize(150, 150, image::imageops::FilterType::Lanczos3);
        thumb.save(&thumb_path).map_err(|e| format!("Failed to save thumbnail: {}", e))?;
    }

    // Build metadata
    let generated_at = chrono::Utc::now().to_rfc3339();
    let meta = GalleryMetadata {
        prompt: prompt.clone(),
        negative_prompt: negative_prompt.clone(),
        model_id: model_id.clone(),
        model_name: model_name.clone(),
        width,
        height,
        steps,
        cfg_scale,
        seed,
        sampler: sampler.clone(),
        generated_at: generated_at.clone(),
        generation_time_secs,
    };

    // Save JSON sidecar
    let json_path = gallery_dir.join(format!("{}.json", id));
    let json_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(&json_path, json_str).map_err(|e| e.to_string())?;

    Ok(GalleryItem {
        id: id.clone(),
        filename: format!("{}.png", id),
        thumbnail_path: thumb_path.to_string_lossy().into_owned(),
        full_path: png_path.to_string_lossy().into_owned(),
        prompt,
        negative_prompt,
        model_id,
        model_name,
        width,
        height,
        steps,
        cfg_scale,
        seed,
        sampler,
        generated_at,
        generation_time_secs,
    })
}

fn rand_suffix() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    (h.finish() % 1_000_000) as u32
}

#[tauri::command]
pub async fn export_image(
    state: State<'_, AppState>,
    item_id: String,
    destination: String,
) -> Result<(), String> {
    let gallery_dir = get_gallery_dir(&state.app_handle)?;
    let source = gallery_dir.join(format!("{}.png", item_id));

    if !source.exists() {
        return Err(format!("Gallery item '{}' not found", item_id));
    }

    std::fs::copy(&source, &destination).map_err(|e| e.to_string())?;
    Ok(())
}
