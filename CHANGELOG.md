# Changelog

## Unreleased

### Fixed
- **Edit Mode crashed the app on Z-Image.** Reference-image editing only works on models
  trained for it (Flux Kontext); sd.cpp does not check, and a reference image on Z-Image
  Turbo took down the process with an access violation. The backend now refuses with a
  clear message, the UI only offers Edit Mode for editing models, and errors from the
  generate command are shown in the app instead of only the console.
- **"Edit this image" button** on generated results. Reaching img2img or Edit Mode
  previously required dragging the result onto its own canvas.

## v0.4.0 — 2026-09-03

Engine upgrade release: stable-diffusion.cpp moved forward six months (to `6b3edaa`),
and the whole app was then verified by driving the real window, which found and fixed
a run of bugs the library tests could not see.

### Engine
- **stable-diffusion.cpp bumped to 6b3edaa** and the FFI migrated to its new API
  (bool-returning generate calls, `free_sd_images`, new upscaler constructor,
  reference-image arguments, `params_backend` / `auto_fit`).
- **Weight placement is decided per load.** Six performance toggles (Flash Attention,
  memory-mapped loading, offload to CPU, free params early, CLIP on CPU, VAE on CPU)
  are gone. sd.cpp's `auto_fit` plans placement, and Blink itself forces the
  low-memory path when the model plus sd.cpp's 2 GB compute reserve will not fit in
  free VRAM, because below that line `auto_fit` moves the diffusion *compute* to the
  CPU and a 4-second image takes 4.5 minutes. One user override remains:
  **Low-memory mode** in Settings.
- **LoRA fix for feed_forward tensors** applied to the vendored sd.cpp at build time,
  so LoRAs on transformer-style models no longer silently drop their FFN weights. The
  build fails loudly if the fix is ever missing.
- **Native cancellation.** Cancel now stops sd.cpp inside the current sampling step
  instead of waiting for the generation to finish, and stays armed across model
  switches.

### Fixed
- The **Cancel button did nothing** (a Solid render-time binding always called the
  generate handler). Verified: a 20-step run stops within the current step.
- **Live previews stalled sampling.** Without a matching TAESD the preview fell back
  to the full VAE, 13 s per preview; a 20-step Z-Image took 134 s instead of 14 s.
  Previews now use sd.cpp's latent projection unless a TAESD is loaded.
- **Live previews never reached the UI** (RGBA frames failed JPEG encoding silently).
  They now show, scaled to the canvas.
- **Model loading is its own progress phase** instead of fake step progress, and the
  progress bar no longer leaks tensor counts into the step counter.
- **Deleted models no longer show as ready**: metadata is reconciled against disk.
- **LoRA picker** uses the native file dialog; Z-Image's 4x LoRA over-application is
  compensated so default strengths look right.
- **Size list** includes Z-Image's 512×1024 default (it rendered blank before) and
  every saved size gets an entry; several Settings widths and fonts that were being
  silently overridden now render as designed.
- **"Preserve Structure (Canny)" removed from Settings**: it was never wired to a
  ControlNet model or a control image, so it did nothing. The backend keeps the slot
  for when it is implemented properly.
- Workspace is clippy-clean; a poisoned-lock edge case that could disarm cancel is
  handled; dead companion-file size estimates removed.

### Since v0.3.0 (earlier, unreleased)
- Prompt and generation info shown on images.
- AI prompt enhancement via the Claude API, or a local LLM through Ollama / LM Studio
  with auto-detected models.
- Smart multi-file model import.

### Known limitations
- Cancel cannot interrupt a model *load* (sd.cpp's loader never checks the flag); it
  takes effect once loading finishes.
- The low-memory fallback below ~9 GB of free VRAM was validated by capping the budget
  on a 12 GB card, not on real 8/6/4 GB hardware.
- Windows ships two installers: `-cuda-setup.exe` (NVIDIA GPU inference, CUDA runtime
  bundled, needs an NVIDIA driver that supports CUDA 13, i.e. R580 or newer) and the plain
  `-setup.exe` (CPU only). Linux installers are CPU only; macOS is Metal.

## v0.3.0 — 2026-03-14

Inpainting, Kontext editing, LoRA, upscaling, video generation, UI overhaul. See the
[GitHub release](https://github.com/BeargleIndustries/blink/releases/tag/v0.3.0).
