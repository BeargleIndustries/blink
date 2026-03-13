use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use crate::error::SdError;
use crate::ffi_bridge::SdCppContext;
use crate::types::*;
use crate::progress::{ProgressCallback, ProgressUpdate};

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
    model_path: String,
    command_tx: mpsc::Sender<InferenceCommand>,
    cancel_flag: Arc<AtomicBool>,
    _thread: thread::JoinHandle<()>,
}

impl SdContext {
    pub fn new(config: ContextConfig) -> Result<Self, SdError> {
        Self::with_cancel_flag(config, Arc::new(AtomicBool::new(false)))
    }

    pub fn with_cancel_flag(config: ContextConfig, cancel_flag: Arc<AtomicBool>) -> Result<Self, SdError> {
        let model_path = config.model_path.clone();

        // Validate model file exists
        if !std::path::Path::new(&config.model_path).exists() {
            return Err(SdError::ModelNotFound { path: config.model_path });
        }
        let (command_tx, command_rx) = mpsc::channel::<InferenceCommand>();

        let thread_config = config.clone();
        let thread_cancel = cancel_flag.clone();

        let handle = thread::Builder::new()
            .name("sd-inference".into())
            .spawn(move || {
                log::info!("Inference thread started for model: {}", thread_config.model_path);

                // Initialize sd.cpp context (loads the model)
                let cpp_ctx = match SdCppContext::new(&thread_config) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::error!("Failed to load model: {}", e);
                        // Drain remaining commands and send errors
                        while let Ok(cmd) = command_rx.try_recv() {
                            match cmd {
                                InferenceCommand::Txt2Img { result_tx, .. } => {
                                    let _ = result_tx.send(Err(SdError::ContextCreationFailed {
                                        reason: format!("Model failed to load: {}", e),
                                    }));
                                }
                                InferenceCommand::Img2Img { result_tx, .. } => {
                                    let _ = result_tx.send(Err(SdError::ContextCreationFailed {
                                        reason: format!("Model failed to load: {}", e),
                                    }));
                                }
                                InferenceCommand::Shutdown => {}
                            }
                        }
                        return;
                    }
                };

                while let Ok(cmd) = command_rx.recv() {
                    match cmd {
                        InferenceCommand::Txt2Img { params, progress_cb, result_tx } => {
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

        Ok(Self {
            model_path,
            command_tx,
            cancel_flag,
            _thread: handle,
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

        result_rx.recv().map_err(|_| SdError::FfiPanic {
            message: "Inference thread crashed during generation".into(),
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

        result_rx.recv().map_err(|_| SdError::FfiPanic {
            message: "Inference thread crashed during generation".into(),
        })?
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

impl Drop for SdContext {
    fn drop(&mut self) {
        log::info!("Dropping SdContext, sending shutdown...");
        let _ = self.command_tx.send(InferenceCommand::Shutdown);
    }
}
