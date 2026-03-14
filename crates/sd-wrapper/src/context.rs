use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
use crate::error::SdError;
use crate::ffi_bridge::SdCppContext;
use crate::types::*;
use crate::progress::ProgressCallback;

pub enum InferenceCommand {
    Txt2Img {
        params: GenerationParams,
        progress_cb: Option<ProgressCallback>,
        result_tx: mpsc::Sender<Result<GeneratedImage, SdError>>,
    },
    Img2Img {
        input_image: Vec<u8>,
        params: Img2ImgParams,
        progress_cb: Option<ProgressCallback>,
        result_tx: mpsc::Sender<Result<GeneratedImage, SdError>>,
    },
    Shutdown,
}

pub struct SdContext {
    model_path: Option<String>,
    command_tx: mpsc::Sender<InferenceCommand>,
    cancel_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl SdContext {
    pub fn new(config: ContextConfig) -> Result<Self, SdError> {
        Self::with_cancel_flag(config, Arc::new(AtomicBool::new(false)))
    }

    pub fn with_cancel_flag(config: ContextConfig, cancel_flag: Arc<AtomicBool>) -> Result<Self, SdError> {
        let model_path = config.model_path.clone();

        // Validate that at least one model path is provided
        if config.model_path.is_none() && config.diffusion_model_path.is_none() {
            return Err(SdError::InvalidParams {
                reason: "No model path provided — set model_path (SD/SDXL) or diffusion_model_path (Flux)".into(),
            });
        }

        // Validate model file exists if provided
        if let Some(ref mp) = config.model_path {
            if !std::path::Path::new(mp).exists() {
                return Err(SdError::ModelNotFound { path: mp.clone() });
            }
        }
        if let Some(ref dp) = config.diffusion_model_path {
            if !std::path::Path::new(dp).exists() {
                return Err(SdError::ModelNotFound { path: dp.clone() });
            }
        }
        let (command_tx, command_rx) = mpsc::channel::<InferenceCommand>();
        // Channel to report whether model loading succeeded or failed
        let (load_tx, load_rx) = mpsc::channel::<Result<(), String>>();

        let thread_config = config.clone();
        let thread_cancel = cancel_flag.clone();

        let handle = thread::Builder::new()
            .name("sd-inference".into())
            .spawn(move || {
                let model_display = thread_config.model_path.as_deref()
                    .or(thread_config.diffusion_model_path.as_deref())
                    .unwrap_or("<none>");
                eprintln!("[blink] Inference thread started, loading model: {}", model_display);

                // Initialize sd.cpp context (loads the model)
                let cpp_ctx = match SdCppContext::new(&thread_config) {
                    Ok(ctx) => {
                        eprintln!("[blink] Model loaded successfully");
                        let _ = load_tx.send(Ok(()));
                        ctx
                    }
                    Err(e) => {
                        eprintln!("[blink] Failed to load model: {}", e);
                        let _ = load_tx.send(Err(format!("{}", e)));
                        return;
                    }
                };

                while let Ok(cmd) = command_rx.recv() {
                    match cmd {
                        InferenceCommand::Txt2Img { params, progress_cb, result_tx } => {
                            log::info!("Starting txt2img: {}x{}, {} steps", params.width, params.height, params.steps);
                            thread_cancel.store(false, Ordering::SeqCst);
                            let result = cpp_ctx.generate(
                                &params,
                                None,   // no input image for txt2img
                                0.0,    // strength unused for txt2img
                                progress_cb,
                                &thread_cancel,
                            );
                            let _ = result_tx.send(result);
                        }
                        InferenceCommand::Img2Img { input_image, params, progress_cb, result_tx } => {
                            thread_cancel.store(false, Ordering::SeqCst);
                            let result = cpp_ctx.generate(
                                &params.base,
                                Some(&input_image),
                                params.strength,
                                progress_cb,
                                &thread_cancel,
                            );
                            let _ = result_tx.send(result);
                        }
                        InferenceCommand::Shutdown => {
                            log::info!("Inference thread shutting down");
                            break;
                        }
                    }
                }

                // cpp_ctx is dropped here, which calls free_sd_ctx via Drop
                log::info!("Inference thread stopped");
            })
            .map_err(|e| SdError::ContextCreationFailed {
                reason: format!("Failed to spawn inference thread: {}", e),
            })?;

        // Wait for model loading result (blocks until the model is loaded or fails)
        match load_rx.recv_timeout(Duration::from_secs(300)) {
            Ok(Ok(())) => {}  // Model loaded successfully
            Ok(Err(e)) => {
                return Err(SdError::ContextCreationFailed {
                    reason: format!("Model failed to load: {}", e),
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(SdError::ContextCreationFailed {
                    reason: "Model loading timed out after 5 minutes".into(),
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(SdError::ContextCreationFailed {
                    reason: "Inference thread crashed during model loading".into(),
                });
            }
        }

        Ok(Self {
            model_path,
            command_tx,
            cancel_flag,
            thread: Some(handle),
        })
    }

    pub fn txt2img(
        &self,
        params: GenerationParams,
        progress_cb: Option<ProgressCallback>,
    ) -> Result<GeneratedImage, SdError> {
        let (result_tx, result_rx) = mpsc::channel();
        self.command_tx
            .send(InferenceCommand::Txt2Img { params, progress_cb, result_tx })
            .map_err(|_| SdError::ContextCreationFailed {
                reason: "Inference thread has stopped".into(),
            })?;

        result_rx.recv_timeout(Duration::from_secs(1800)).map_err(|e| match e {
            mpsc::RecvTimeoutError::Timeout => SdError::FfiPanic {
                message: "Generation timed out after 30 minutes".into(),
            },
            mpsc::RecvTimeoutError::Disconnected => SdError::FfiPanic {
                message: "Inference thread crashed during generation".into(),
            },
        })?
    }

    pub fn img2img(
        &self,
        input_image: Vec<u8>,
        params: Img2ImgParams,
        progress_cb: Option<ProgressCallback>,
    ) -> Result<GeneratedImage, SdError> {
        let (result_tx, result_rx) = mpsc::channel();
        self.command_tx
            .send(InferenceCommand::Img2Img { input_image, params, progress_cb, result_tx })
            .map_err(|_| SdError::ContextCreationFailed {
                reason: "Inference thread has stopped".into(),
            })?;

        result_rx.recv_timeout(Duration::from_secs(1800)).map_err(|e| match e {
            mpsc::RecvTimeoutError::Timeout => SdError::FfiPanic {
                message: "Generation timed out after 30 minutes".into(),
            },
            mpsc::RecvTimeoutError::Disconnected => SdError::FfiPanic {
                message: "Inference thread crashed during generation".into(),
            },
        })?
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }
}

impl Drop for SdContext {
    fn drop(&mut self) {
        log::info!("Dropping SdContext, sending shutdown...");
        let _ = self.command_tx.send(InferenceCommand::Shutdown);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
