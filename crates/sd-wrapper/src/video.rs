//! Video generation (Wan/CogVideo) via sd.cpp `generate_video`.

use std::ffi::{c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::SdError;
use crate::ffi_bridge::SdCppContext;
use crate::progress::{ProgressCallback, ProgressUpdate};
use crate::types::GeneratedImage;

#[derive(Debug, Clone)]
pub struct VideoGenParams {
    pub prompt: String,
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    /// Denoising strength (used when init_image is provided).
    pub strength: f32,
    pub seed: i64,
    /// Number of frames to generate.
    pub video_frames: u32,
    /// Optional init image (PNG/JPEG bytes). When provided, used as the first frame reference.
    pub input_image: Option<Vec<u8>>,
}

impl Default for VideoGenParams {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            width: 512,
            height: 512,
            strength: 1.0,
            seed: -1,
            video_frames: 16,
            input_image: None,
        }
    }
}

/// Generate video frames using sd.cpp's `generate_video`.
///
/// Returns one `GeneratedImage` per frame (RGBA, same dimensions).
/// The caller is responsible for encoding the frames (e.g. to GIF).
pub(crate) fn generate_video(
    cpp_ctx: &SdCppContext,
    params: &VideoGenParams,
    progress_cb: Option<ProgressCallback>,
    cancel_flag: &AtomicBool,
) -> Result<Vec<GeneratedImage>, SdError> {
    if cancel_flag.load(Ordering::SeqCst) {
        return Err(SdError::Cancelled);
    }

    if params.width == 0 || params.height == 0 {
        return Err(SdError::InvalidParams {
            reason: "width and height must be > 0".into(),
        });
    }
    if params.video_frames == 0 {
        return Err(SdError::InvalidParams {
            reason: "video_frames must be > 0".into(),
        });
    }

    let prompt_c = CString::new(params.prompt.as_str()).map_err(|_| SdError::InvalidParams {
        reason: "prompt contains interior NUL byte".into(),
    })?;
    let neg_prompt_c = CString::new(params.negative_prompt.as_str()).map_err(|_| SdError::InvalidParams {
        reason: "negative_prompt contains interior NUL byte".into(),
    })?;

    // Decode optional init image to raw RGB
    let mut decoded_rgb: Vec<u8> = Vec::new();
    let mut init_image = sd_sys::sd_image_t {
        width: 0,
        height: 0,
        channel: 0,
        data: std::ptr::null_mut(),
    };

    if let Some(ref img_bytes) = params.input_image {
        let img = image::load_from_memory(img_bytes).map_err(|e| SdError::InvalidParams {
            reason: format!("Failed to decode input image: {}", e),
        })?;
        let align = 64u32;
        let aligned_w = ((img.width() + align - 1) / align) * align;
        let aligned_h = ((img.height() + align - 1) / align) * align;
        let resized = img.resize_exact(aligned_w, aligned_h, image::imageops::FilterType::Lanczos3);
        let rgb_img = resized.to_rgb8();
        let (w, h) = rgb_img.dimensions();
        decoded_rgb = rgb_img.into_raw();
        init_image = sd_sys::sd_image_t {
            width: w,
            height: h,
            channel: 3,
            data: decoded_rgb.as_mut_ptr(),
        };
    }

    // Heap-allocate progress trampoline data
    let trampoline_data = Box::new(VideoProgressTrampolineData { callback: progress_cb });
    let trampoline_ptr = Box::into_raw(trampoline_data);

    unsafe {
        sd_sys::sd_set_progress_callback(
            Some(video_progress_trampoline),
            trampoline_ptr as *mut c_void,
        );

        let mut vid_params: sd_sys::sd_vid_gen_params_t = std::mem::zeroed();
        sd_sys::sd_vid_gen_params_init(&mut vid_params);

        vid_params.prompt = prompt_c.as_ptr();
        vid_params.negative_prompt = neg_prompt_c.as_ptr();
        vid_params.width = params.width as i32;
        vid_params.height = params.height as i32;
        vid_params.seed = params.seed;
        vid_params.video_frames = params.video_frames as i32;
        vid_params.strength = params.strength;

        // Set init_image if one was decoded
        if !init_image.data.is_null() {
            vid_params.init_image = init_image;
        }

        log::info!(
            "Calling generate_video: prompt='{}', {}x{}, {} frames, seed={}",
            params.prompt,
            params.width,
            params.height,
            params.video_frames,
            params.seed,
        );

        let mut num_frames_out: std::ffi::c_int = 0;
        let result_ptr = sd_sys::generate_video(cpp_ctx.raw_ptr(), &vid_params, &mut num_frames_out);

        // Clear progress callback before dropping trampoline
        sd_sys::sd_set_progress_callback(None, std::ptr::null_mut());
        let _ = Box::from_raw(trampoline_ptr);

        if result_ptr.is_null() || num_frames_out <= 0 {
            return Err(SdError::InferenceReturnedNull);
        }

        let frame_count = num_frames_out as usize;
        let mut frames = Vec::with_capacity(frame_count);

        for i in 0..frame_count {
            let sd_img = &*result_ptr.add(i);
            let w = sd_img.width;
            let h = sd_img.height;
            let ch = sd_img.channel;
            let pixel_count = (w as usize) * (h as usize) * (ch as usize);

            if sd_img.data.is_null() || pixel_count == 0 {
                eprintln!("[blink] Video frame {} has null/empty data, skipping", i);
                continue;
            }

            let src_slice = std::slice::from_raw_parts(sd_img.data, pixel_count);

            let rgba_data = if ch == 3 {
                let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
                for pixel in src_slice.chunks_exact(3) {
                    rgba.push(pixel[0]);
                    rgba.push(pixel[1]);
                    rgba.push(pixel[2]);
                    rgba.push(255);
                }
                rgba
            } else {
                src_slice.to_vec()
            };

            // Free each frame's pixel data (allocated by sd.cpp via malloc)
            libc::free(sd_img.data as *mut c_void);

            frames.push(GeneratedImage {
                data: rgba_data,
                width: w,
                height: h,
            });
        }

        // Free the frame array itself (allocated by sd.cpp via operator new[])
        libc::free(result_ptr as *mut c_void);

        if frames.is_empty() {
            return Err(SdError::InferenceReturnedNull);
        }

        Ok(frames)
    }
}

struct VideoProgressTrampolineData {
    callback: Option<ProgressCallback>,
}

unsafe extern "C" fn video_progress_trampoline(
    step: std::ffi::c_int,
    steps: std::ffi::c_int,
    time: f32,
    data: *mut c_void,
) {
    eprintln!("[blink] Video progress: step={}, total={}, time={:.1}s", step, steps, time);
    if data.is_null() {
        return;
    }
    let cb_data = &*(data as *const VideoProgressTrampolineData);
    if let Some(ref cb) = cb_data.callback {
        cb(ProgressUpdate {
            step: step as u32,
            total_steps: steps as u32,
            elapsed_secs: time,
            preview: None,
        });
    }
}
