use tauri::{State, Emitter};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use sd_wrapper::{GenerationParams, Img2ImgParams, SampleMethod, ProgressUpdate, LoraConfig, PreviewCallback};
use base64::Engine;

#[derive(Debug, Deserialize)]
pub struct LoraRequest {
    pub path: String,
    pub multiplier: f32,
}

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
    pub mask_image: Option<Vec<u8>>,
    pub strength: Option<f32>,
    pub ref_images: Option<Vec<Vec<u8>>>,
    pub img_cfg: Option<f32>,
    pub loras: Option<Vec<LoraRequest>>,
    pub control_strength: Option<f32>,
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

    let loras: Vec<LoraConfig> = request.loras.unwrap_or_default()
        .into_iter()
        .map(|l| LoraConfig { path: l.path, multiplier: l.multiplier, is_high_noise: false })
        .collect();

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
        ref_images: request.ref_images.clone().unwrap_or_default(),
        img_cfg: request.img_cfg,
        loras,
    };

    let is_img2img = request.input_image.is_some();
    let input_image = request.input_image;
    let mask_image = request.mask_image;
    let strength = request.strength.unwrap_or(0.75);
    let control_strength = request.control_strength;
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

    // Set up preview callback that emits Tauri events with JPEG-encoded preview frames
    let preview_handle = app_handle.clone();
    let preview_cb: Option<PreviewCallback> = Some(Box::new(move |_step: i32, image_data: Vec<u8>, width: u32, height: u32| {
        use image::{ImageBuffer, RgbaImage};
        if let Some(img) = ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, image_data) {
            let img: RgbaImage = img;
            let mut jpeg_buf = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut jpeg_buf);
            if img.write_to(&mut cursor, image::ImageFormat::Jpeg).is_ok() {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_buf);
                let _ = preview_handle.emit("generation:preview", serde_json::json!({
                    "image_base64": b64,
                    "width": width,
                    "height": height,
                }));
            }
        }
    }));

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
            ctx.img2img(input_image.unwrap_or_default(), mask_image, img_params, None, control_strength, Some(progress_cb), preview_cb)
        } else {
            ctx.txt2img(params.clone(), params.ref_images.clone(), Some(progress_cb), preview_cb)
        }
    };

    state.generating.store(false, Ordering::SeqCst);

    match result {
        Ok(image) => {
            if state.cancel_flag.load(Ordering::SeqCst) {
                state.generating.store(false, Ordering::SeqCst);
                let _ = app_handle.emit("generation:cancelled", ());
                return Err("Generation cancelled".into());
            }

            let elapsed = start.elapsed().as_secs_f32();

            log::info!(
                "Image generated: {}x{}, data len={}, first 16 bytes={:?}",
                image.width, image.height, image.data.len(),
                &image.data[..image.data.len().min(16)]
            );

            let png_data = match encode_image_to_png(&image.data, image.width, image.height) {
                Ok(data) => data,
                Err(e) => {
                    let _ = app_handle.emit("generation:error", GenerationErrorEvent {
                        message: e.clone(),
                        recovery: None,
                    });
                    return Err(e);
                }
            };
            log::info!("PNG encoded: {} bytes", png_data.len());
            let base64_image = base64_encode(&png_data);
            log::info!("Base64 length: {}", base64_image.len());

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
    state.generating.store(false, Ordering::SeqCst);
    Ok(())
}

const ENHANCE_SYSTEM_PROMPT: &str = r#"You are an expert AI image generation prompt engineer. Enhance the user's simple prompt into a detailed, professional image generation prompt, and generate an appropriate negative prompt.

Positive prompt rules:
- Keep the core subject/intent of the original prompt
- Add specific visual details: materials, textures, colors, lighting
- Add composition details: camera angle, lens type, depth of field
- Add mood/atmosphere: time of day, weather, emotional tone
- Add style keywords: photorealistic, cinematic, detailed, sharp focus
- Add technical quality boosters: 8k, high resolution, detailed textures
- Keep it under 200 words
- Match the style to the content (photorealistic for photos, artistic terms for art, etc.)

Negative prompt rules:
- Include common quality issues to avoid: blurry, low quality, distorted, deformed
- Add content-appropriate exclusions (e.g., for portraits: extra limbs, bad anatomy, bad hands)
- Keep it concise — 10-30 words max

Output format — use EXACTLY this format:
PROMPT: [enhanced prompt]
NEGATIVE: [negative prompt]

Output ONLY these two lines. No explanation, no quotes, no other text."#;

fn parse_enhance_response(text: String) -> EnhancedPrompt {
    let mut enhanced_prompt = text.clone();
    let mut negative_prompt = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(p) = line.strip_prefix("PROMPT:") {
            enhanced_prompt = p.trim().to_string();
        } else if let Some(n) = line.strip_prefix("NEGATIVE:") {
            negative_prompt = n.trim().to_string();
        }
    }
    EnhancedPrompt { prompt: enhanced_prompt, negative_prompt }
}

#[derive(serde::Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(serde::Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[derive(Serialize)]
pub struct EnhancedPrompt {
    pub prompt: String,
    pub negative_prompt: String,
}

#[tauri::command]
pub async fn enhance_prompt(prompt: String, api_key: String) -> Result<EnhancedPrompt, String> {
    let client = reqwest::Client::new();

    let system_prompt = ENHANCE_SYSTEM_PROMPT;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 400,
            "system": system_prompt,
            "messages": [{
                "role": "user",
                "content": format!("Enhance this image generation prompt:\n\n{}", prompt)
            }]
        }))
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let result: AnthropicResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let text = result.content
        .first()
        .and_then(|block| block.text.clone())
        .ok_or_else(|| "No text in response".to_string())?;

    Ok(parse_enhance_response(text))
}

async fn enhance_prompt_ollama_native(prompt: String, endpoint: String, model: String) -> Result<EnhancedPrompt, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/chat", endpoint.trim_end_matches('/'));

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": ENHANCE_SYSTEM_PROMPT},
                {"role": "user", "content": format!("Enhance this image generation prompt:\n\n{}", prompt)}
            ],
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}. Is Ollama running?", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama error {}: {}", status, body));
    }

    let result: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let text = result["message"]["content"]
        .as_str()
        .ok_or_else(|| "No content in Ollama response".to_string())?
        .to_string();

    Ok(parse_enhance_response(text))
}

#[tauri::command]
pub async fn enhance_prompt_local(prompt: String, endpoint: String, model: String) -> Result<EnhancedPrompt, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));

    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": ENHANCE_SYSTEM_PROMPT},
                {"role": "user", "content": format!("Enhance this image generation prompt:\n\n{}", prompt)}
            ],
            "max_tokens": 400,
            "temperature": 0.7
        }))
        .send()
        .await
        .map_err(|e| format!("Local LLM request failed: {}. Is Ollama/LM Studio running?", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        // If 404, try Ollama native format
        if status.as_u16() == 404 {
            return enhance_prompt_ollama_native(prompt, endpoint, model).await;
        }
        return Err(format!("Local LLM error {}: {}", status, body));
    }

    let result: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let text = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "No content in response".to_string())?
        .to_string();

    Ok(parse_enhance_response(text))
}

fn encode_image_to_png(rgba_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, RgbaImage};
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
        .ok_or_else(|| format!("Invalid image dimensions: {}x{} with {} bytes", width, height, rgba_data.len()))?;
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    Ok(buf)
}

fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}
