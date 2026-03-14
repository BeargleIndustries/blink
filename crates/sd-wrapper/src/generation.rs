use crate::context::SdContext;
use crate::error::SdError;
use crate::types::*;
use crate::progress::ProgressCallback;

/// Public convenience function: text-to-image via an existing SdContext.
pub fn generate_txt2img(
    ctx: &SdContext,
    params: &GenerationParams,
    ref_images: Option<&[Vec<u8>]>,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    let refs = ref_images.map(|r| r.to_vec()).unwrap_or_default();
    ctx.txt2img(params.clone(), refs, progress_cb, None)
}

/// Public convenience function: image-to-image via an existing SdContext.
pub fn generate_img2img(
    ctx: &SdContext,
    input_image: &[u8],
    mask_image: Option<&[u8]>,
    params: &Img2ImgParams,
    control_image: Option<&[u8]>,
    control_strength: Option<f32>,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    ctx.img2img(
        input_image.to_vec(),
        mask_image.map(|m| m.to_vec()),
        params.clone(),
        control_image.map(|c| c.to_vec()),
        control_strength,
        progress_cb,
        None,
    )
}
