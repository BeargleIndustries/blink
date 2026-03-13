# Blink

## Project Context
Dead-simple local AI image generation app. Native cross-platform desktop tool — type a prompt, get an image. No Python, no node graphs.
Vault overview: `D:\BeargleVault\projects\blink\blink.md`

## Tech Stack
- Tauri v2 (Rust backend, TypeScript/HTML frontend)
- stable-diffusion.cpp via FFI (inference engine, no Python)
- GGUF quantized models preferred
- Supports CUDA, Vulkan, Metal, OpenCL, CPU fallback

## Conventions
- Rust for backend/inference layer, TypeScript for UI
- Keep the UI dead simple — complexity hidden behind "Advanced" toggles
- Model browser is the killer feature — prioritize its UX
- Open source project, not monetized

## Key Paths
- `src-tauri/` — Rust backend (Tauri commands, sd.cpp FFI)
- `src/` — Frontend (TypeScript/HTML)
- `models/` — Local model storage (gitignored)

## When Working on This Project
- No Python dependencies — the whole point is zero-Python install
- Target users are non-technical — every UX decision should favor simplicity
- sd.cpp supports SD 1.5, SDXL, Flux, Wan, and more
- Check vault architecture notes for VRAM requirements and model format details
