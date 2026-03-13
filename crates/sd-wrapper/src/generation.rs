use crate::context::SdContext;
use crate::error::SdError;
use crate::types::*;
use crate::progress::ProgressCallback;

/// Public convenience function: text-to-image via an existing SdContext.
pub fn generate_txt2img(
    ctx: &SdContext,
    params: &GenerationParams,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    ctx.txt2img(params.clone(), progress_cb)
}

/// Public convenience function: image-to-image via an existing SdContext.
pub fn generate_img2img(
    ctx: &SdContext,
    input_image: &[u8],
    params: &Img2ImgParams,
    progress_cb: Option<ProgressCallback>,
) -> Result<GeneratedImage, SdError> {
    ctx.img2img(input_image.to_vec(), params.clone(), progress_cb)
}
