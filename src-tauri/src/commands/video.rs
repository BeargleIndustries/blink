use tauri::{State, Emitter};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use sd_wrapper::{VideoGenParams, ProgressUpdate};
use base64::Engine;

#[derive(Debug, Deserialize)]
pub struct VideoRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub seed: Option<i64>,
    pub video_frames: Option<u32>,
    pub strength: Option<f32>,
    pub input_image: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VideoCompleteEvent {
    pub gif_base64: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub generation_time_secs: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct VideoErrorEvent {
    pub message: String,
}

#[tauri::command]
pub async fn generate_video(
    state: State<'_, AppState>,
    request: VideoRequest,
) -> Result<String, String> {
    if state.generating.load(Ordering::SeqCst) {
        return Err("Generation already in progress".into());
    }

    {
        let ctx_lock = state.sd_context.lock().map_err(|e| e.to_string())?;
        if ctx_lock.is_none() {
            return Err("No model loaded. Please download and select a model first.".into());
        }
    }

    state.cancel_flag.store(false, Ordering::SeqCst);
    state.generating.store(true, Ordering::SeqCst);

    let app_handle = state.app_handle.clone();

    let params = VideoGenParams {
        prompt: request.prompt,
        negative_prompt: request.negative_prompt.unwrap_or_default(),
        width: request.width.unwrap_or(512),
        height: request.height.unwrap_or(512),
        seed: request.seed.unwrap_or(-1),
        video_frames: request.video_frames.unwrap_or(16),
        strength: request.strength.unwrap_or(1.0),
        input_image: request.input_image,
    };

    let progress_handle = app_handle.clone();
    let progress_cb: sd_wrapper::progress::ProgressCallback = Box::new(move |update: ProgressUpdate| {
        let _ = progress_handle.emit("generation:progress", serde_json::json!({
            "step": update.step,
            "total_steps": update.total_steps,
            "elapsed_secs": update.elapsed_secs,
        }));
    });

    let start = std::time::Instant::now();

    let result = {
        let ctx_lock = state.sd_context.lock().map_err(|e| {
            state.generating.store(false, Ordering::SeqCst);
            e.to_string()
        })?;

        let ctx = ctx_lock.as_ref().ok_or_else(|| {
            state.generating.store(false, Ordering::SeqCst);
            "No model loaded".to_string()
        })?;

        ctx.generate_video(params, Some(progress_cb))
    };

    state.generating.store(false, Ordering::SeqCst);

    match result {
        Ok(frames) => {
            if state.cancel_flag.load(Ordering::SeqCst) {
                let _ = app_handle.emit("generation:cancelled", ());
                return Err("Generation cancelled".into());
            }

            let elapsed = start.elapsed().as_secs_f32();
            let frame_count = frames.len() as u32;

            if frames.is_empty() {
                let _ = app_handle.emit("video:error", VideoErrorEvent {
                    message: "No frames generated".into(),
                });
                return Err("No frames generated".into());
            }

            let width = frames[0].width;
            let height = frames[0].height;

            let gif_data = encode_frames_to_gif(frames)
                .map_err(|e| {
                    let _ = app_handle.emit("video:error", VideoErrorEvent { message: e.clone() });
                    e
                })?;

            let gif_base64 = base64::engine::general_purpose::STANDARD.encode(&gif_data);

            let _ = app_handle.emit("video:complete", VideoCompleteEvent {
                gif_base64: gif_base64.clone(),
                width,
                height,
                frame_count,
                generation_time_secs: elapsed,
            });

            Ok(gif_base64)
        }
        Err(sd_wrapper::SdError::Cancelled) => {
            let _ = app_handle.emit("generation:cancelled", ());
            Err("Generation cancelled".into())
        }
        Err(e) => {
            let _ = app_handle.emit("video:error", VideoErrorEvent { message: e.to_string() });
            Err(e.to_string())
        }
    }
}

/// Encode a sequence of RGBA frames into a GIF at ~8fps (125ms per frame).
fn encode_frames_to_gif(frames: Vec<sd_wrapper::GeneratedImage>) -> Result<Vec<u8>, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Frame, RgbaImage, ImageBuffer};

    let mut buf = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut buf);
        encoder.set_repeat(Repeat::Infinite)
            .map_err(|e| format!("Failed to set GIF repeat: {}", e))?;

        for frame_img in frames.into_iter() {
            let w = frame_img.width;
            let h = frame_img.height;
            let data_len = frame_img.data.len();
            let rgba: RgbaImage = ImageBuffer::from_raw(w, h, frame_img.data)
            .ok_or_else(|| format!(
                "Invalid frame dimensions: {}x{} with {} bytes",
                w, h, data_len
            ))?;

            // ~8fps: 125ms per frame
            let frame = Frame::from_parts(
                rgba,
                0,
                0,
                image::Delay::from_numer_denom_ms(125, 1),
            );

            encoder.encode_frame(frame)
                .map_err(|e| format!("Failed to encode GIF frame: {}", e))?;
        }
    }

    Ok(buf)
}
