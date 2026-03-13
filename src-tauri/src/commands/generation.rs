use tauri::{State, Emitter};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use sd_wrapper::{GenerationParams, Img2ImgParams, SampleMethod, ProgressUpdate};
use base64::Engine;

#[derive(Debug, Deserialize)]
pub struct GenerationRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub steps: Option<u32>,
    pub cfg_scale: Option<f32>,
    pub seed: Option<i64>,
    pub sampler: Option<String>,
    pub input_image: Option<Vec<u8>>,
    pub strength: Option<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GenerationProgressEvent {
    pub step: u32,
    pub total_steps: u32,
    pub elapsed_secs: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct GenerationCompleteEvent {
    pub image_base64: String,
    pub width: u32,
    pub height: u32,
    pub seed: i64,
    pub generation_time_secs: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct GenerationErrorEvent {
    pub message: String,
    pub recovery: Option<String>,
}

fn parse_sampler(s: &str) -> SampleMethod {
    match s {
        "euler" => SampleMethod::Euler,
        "euler_a" => SampleMethod::EulerA,
        "heun" => SampleMethod::Heun,
        "dpm2" => SampleMethod::Dpm2,
        "dpm++2m" => SampleMethod::DpmPlusPlus2m,
        "lcm" => SampleMethod::Lcm,
        _ => SampleMethod::EulerA,
    }
}

#[tauri::command]
pub async fn generate_image(
    state: State<'_, AppState>,
    request: GenerationRequest,
) -> Result<String, String> {
    if state.generating.load(Ordering::SeqCst) {
        return Err("Generation already in progress".into());
    }

    // Check if we have a model loaded
    {
        let ctx_lock = state.sd_context.lock().map_err(|e| e.to_string())?;
        if ctx_lock.is_none() {
            return Err("No model loaded. Please download and select a model first.".into());
        }
    }

    state.cancel_flag.store(false, Ordering::SeqCst);
    state.generating.store(true, Ordering::SeqCst);

    let app_handle = state.app_handle.clone();
    let cancel_flag = state.cancel_flag.clone();

    let params = GenerationParams {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone().unwrap_or_default(),
        width: request.width.unwrap_or(512),
        height: request.height.unwrap_or(512),
        steps: request.steps.unwrap_or(20),
        cfg_scale: request.cfg_scale.unwrap_or(7.0),
        seed: request.seed.unwrap_or(-1),
        sample_method: parse_sampler(request.sampler.as_deref().unwrap_or("euler_a")),
        batch_count: 1,
    };

    let is_img2img = request.input_image.is_some();
    let input_image = request.input_image;
    let strength = request.strength.unwrap_or(0.75);
    let seed = params.seed;

    // Set up progress callback that emits Tauri events
    let progress_handle = app_handle.clone();
    let progress_cb: sd_wrapper::progress::ProgressCallback = Box::new(move |update: ProgressUpdate| {
        let _ = progress_handle.emit("generation:progress", GenerationProgressEvent {
            step: update.step,
            total_steps: update.total_steps,
            elapsed_secs: update.elapsed_secs,
        });
    });

    // Run generation — delegates to the inference thread internally
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

        if is_img2img {
            let img_params = Img2ImgParams {
                base: params.clone(),
                strength,
            };
            ctx.img2img(input_image.unwrap_or_default(), img_params, Some(progress_cb))
        } else {
            ctx.txt2img(params.clone(), Some(progress_cb))
        }
    };

    state.generating.store(false, Ordering::SeqCst);

    match result {
        Ok(image) => {
            let elapsed = start.elapsed().as_secs_f32();

            let png_data = encode_image_to_png(&image.data, image.width, image.height);
            let base64_image = base64_encode(&png_data);

            let _ = app_handle.emit("generation:complete", GenerationCompleteEvent {
                image_base64: base64_image.clone(),
                width: image.width,
                height: image.height,
                seed,
                generation_time_secs: elapsed,
            });

            Ok(base64_image)
        }
        Err(sd_wrapper::SdError::Cancelled) => {
            let _ = app_handle.emit("generation:cancelled", ());
            Err("Generation cancelled".into())
        }
        Err(e) => {
            let error_event = GenerationErrorEvent {
                message: e.to_string(),
                recovery: None,
            };
            let _ = app_handle.emit("generation:error", error_event);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn cancel_generation(
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Shared AtomicBool flag — SdContext uses the same Arc, no mutex needed
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(())
}

fn encode_image_to_png(rgba_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    use image::{ImageBuffer, RgbaImage};
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
        .expect("Invalid image dimensions");
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode PNG");
    buf
}

fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}
