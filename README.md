# Blink

Dead-simple local AI image generation. Type a prompt, get an image. No Python. No node graphs. No PhD required.

![Screenshot placeholder](docs/screenshot.png)

## Features

- GPU-accelerated inference (CUDA, Metal, Vulkan)
- Multiple model architectures (SD 1.5, SDXL, Flux, Z-Image Turbo)
- Built-in model browser with one-click downloads from HuggingFace
- Custom model import from any HuggingFace URL
- Image-to-image generation
- Generation gallery with auto-save
- Performance settings (flash attention, memory-mapped loading, VRAM management)
- Dark, minimal UI

## Supported Models

| Model | Architecture | Size | VRAM | Default Steps | License |
|-------|-------------|------|------|---------------|---------|
| Stable Diffusion 1.5 | SD 1.5 | 1.6 GB | 1.6 GB | 20 | CreativeML Open RAIL-M |
| Stable Diffusion 1.5 (Q8) | SD 1.5 | 2.1 GB | 2 GB | 20 | CreativeML Open RAIL-M |
| Stable Diffusion XL | SDXL | 3.6 GB | 5 GB | 25 | CreativeML Open RAIL++-M |
| Flux.1 Schnell | Flux | 9.9 GB | 10 GB | 4 | Apache 2.0 |
| Flux.1 Schnell (Q8) | Flux | 16 GB | 16 GB | 4 | Apache 2.0 |
| Flux.1 Dev | Flux | 10.1 GB | 12 GB | 25 | FLUX.1 Dev Non-Commercial |
| Z-Image Turbo | Z-Image (Lumina2) | 5.5 GB | 4 GB | 4 | Apache 2.0 |
| Z-Image Turbo (Q8) | Z-Image (Lumina2) | 9.2 GB | 8 GB | 4 | Apache 2.0 |

> Flux and Z-Image Turbo are multi-file models (diffusion model + text encoder + VAE). Blink downloads and manages all components automatically.

## System Requirements

- Windows 10+ / macOS 12+ / Linux (Ubuntu 22.04+)
- 8 GB RAM minimum
- GPU recommended:
  - NVIDIA: GTX 1060+ with CUDA Toolkit 12+
  - Apple: M1+ (Metal)
  - AMD: Vulkan SDK
- CPU-only mode available (significantly slower)

## Installation

### Download Release

Download the latest release from [Releases](../../releases).

### Build from Source

#### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) 18+
- [CMake](https://cmake.org/) 3.20+
- [Ninja](https://ninja-build.org/) build system
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with MSVC C++ workload
- Optional: [CUDA Toolkit](https://developer.nvidia.com/cuda-toolkit) 12+ (NVIDIA GPU acceleration)

#### Clone and Build

```bash
git clone --recursive https://github.com/beargleindustries/blink.git
cd blink
npm install
npx tauri build
```

The built binary will be in `src-tauri/target/release/`.

#### Development

```bash
npm install
npx tauri dev
```

#### Build with GPU acceleration

```bash
# CUDA (NVIDIA)
npx tauri build --features cuda

# Metal (Apple Silicon)
npx tauri build --features metal

# Vulkan (AMD / cross-platform)
npx tauri build --features vulkan
```

## Configuration

### HuggingFace Token

Some models require a HuggingFace account. Set your token in **Advanced Settings** to enable downloads for gated models.

### Performance Settings

- **Flash Attention** — Faster inference, lower memory usage. Enabled by default.
- **Memory-Mapped Loading** — Faster model load times. Enabled by default.
- **Offload to CPU** — For models that exceed available VRAM. Recommended for Flux and Z-Image Turbo on lower-end GPUs.

## Tech Stack

- [Tauri v2](https://tauri.app/) — Native desktop framework (Rust + web frontend)
- [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) — Inference engine (no Python)
- [SolidJS](https://www.solidjs.com/) — Frontend UI
- [ggml](https://github.com/ggerganov/ggml) — Tensor library underlying sd.cpp

## License

MIT — see [LICENSE](LICENSE)

## Credits

Built by [Beargle Industries](https://beargleindustries.com).

### Third-Party

- [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) by leejet (MIT)
- [ggml](https://github.com/ggerganov/ggml) by Georgi Gerganov (MIT)
- Model weights sourced from HuggingFace — see individual model licenses in the Model Browser
