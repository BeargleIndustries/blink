use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::context::SdContext;
use crate::error::SdError;
use crate::types::*;
use crate::progress::{ProgressCallback, ProgressUpdate};

/// Called from the inference thread for txt2img
pub(crate) fn generate_txt2img_internal(
    config: &ContextConfig,
    params: &GenerationParams,
    progress_cb: Option<ProgressCallback>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<GeneratedImage, SdError> {
    log::info!("Generating txt2img: prompt='{}', {}x{}, {} steps",
        params.prompt, params.width, params.height, params.steps);

    // Simulate generation with progress callbacks (TODO: replace with real FFI)
    for step in 0..params.steps {
        // Check cancellation between steps
        if cancel_flag.load(Ordering::SeqCst) {
            log::info!("Generation cancelled at step {}/{}", step, params.steps);
            return Err(SdError::Cancelled);
        }

        if let Some(ref cb) = progress_cb {
            cb(ProgressUpdate {
                step: step + 1,
                total_steps: params.steps,
                elapsed_secs: 0.0, // TODO: actual timing
                preview: None,
            });
        }

        // Simulate step time (remove when FFI is wired up)
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // TODO: Replace with actual sd.cpp FFI call via diffusion-rs-sys
    // let result = unsafe { diffusion_rs_sys::txt2img(ctx, ...) };
    // if result.is_null() { return Err(SdError::InferenceReturnedNull); }

    let pixel_count = (params.width * params.height * 4) as usize;
    Ok(GeneratedImage {
        data: vec![128u8; pixel_count], // Gray placeholder
        width: params.width,
        height: params.height,
    })
}

/// Called from the inference thread for img2img
pub(crate) fn generate_img2img_internal(
    config: &ContextConfig,
    input_image: &[u8],
    params: &Img2ImgParams,
    progress_cb: Option<ProgressCallback>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<GeneratedImage, SdError> {
    log::info!("Generating img2img: prompt='{}', strength={}",
        params.base.prompt, params.strength);

    for step in 0..params.base.steps {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(SdError::Cancelled);
        }

        if let Some(ref cb) = progress_cb {
            cb(ProgressUpdate {
                step: step + 1,
                total_steps: params.base.steps,
                elapsed_secs: 0.0,
                preview: None,
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let pixel_count = (params.base.width * params.base.height * 4) as usize;
    Ok(GeneratedImage {
        data: vec![128u8; pixel_count],
        width: params.base.width,
        height: params.base.height,
    })
}

// Keep the public API functions that delegate to SdContext
pub fn generate_txt2img(
    ctx: &SdContext,
    params: &GenerationParams,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    ctx.txt2img(params.clone(), progress_cb)
}

pub fn generate_img2img(
    ctx: &SdContext,
    input_image: &[u8],
    params: &Img2ImgParams,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    ctx.img2img(input_image.to_vec(), params.clone(), progress_cb)
}
