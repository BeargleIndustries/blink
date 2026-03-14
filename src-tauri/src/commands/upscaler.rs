use tauri::State;
use crate::state::AppState;
use base64::Engine;
use sd_wrapper::UpscalerContext;

#[tauri::command]
pub async fn load_upscaler(
    model_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);

    let ctx = UpscalerContext::new(&model_path, n_threads)
        .map_err(|e| e.to_string())?;

    let mut lock = state.upscaler.lock().map_err(|e| e.to_string())?;
    *lock = Some(ctx);
    Ok(())
}

#[tauri::command]
pub fn upscale_image(
    image_base64: String,
    factor: u32,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if factor < 1 || factor > 4 {
        return Err("Upscale factor must be between 1 and 4".to_string());
    }

    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("Failed to decode base64 image: {}", e))?;

    // Synchronous command — Tauri runs this on a blocking thread.
    // Lock is acquired, FFI call runs, lock is released — no async executor involved.
    let lock = state.upscaler.lock().map_err(|e| e.to_string())?;
    let upscaler = lock.as_ref().ok_or("No upscaler loaded")?;
    let result = upscaler.upscale(&image_bytes, factor).map_err(|e| e.to_string())?;

    // Encode result as PNG then base64
    use image::{ImageBuffer, RgbaImage};
    let img: RgbaImage = ImageBuffer::from_raw(result.width, result.height, result.data)
        .ok_or_else(|| format!("Invalid upscaled image dimensions: {}x{}", result.width, result.height))?;
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode upscaled PNG: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

#[tauri::command]
pub async fn unload_upscaler(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut lock = state.upscaler.lock().map_err(|e| e.to_string())?;
    *lock = None;
    Ok(())
}
