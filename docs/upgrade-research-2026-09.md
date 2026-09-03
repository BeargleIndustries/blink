# Blink Modernization Research — September 2026

Research snapshot comparing Blink's pinned stable-diffusion.cpp against upstream `master`.

## 1. Where Blink stands

| Thing | Current state |
|---|---|
| sd.cpp submodule | `d6dd6d7` (2026-03-10) — **316 commits behind** `origin/master` |
| Curated catalog (`models.json`) | 17 entries; newest architectures are Z-Image + FLUX.1-Kontext |
| Samplers exposed in UI | 5 (`euler`, `euler_a`, `heun`, `dpm2`, `dpm++2m`) |
| Samplers available in sd.cpp | **21** |
| Schedulers exposed | 0 — hardcoded Karras-or-model-default in `ffi_bridge.rs:393-401` |
| Video | Wan 2.1 T2V 1.3B only |

## 2. What landed upstream since March 2026

**New image models:** Lens / Lens Turbo, PiD (NVIDIA Pixel Diffusion Decoder), LongCat Image,
MiniT2I, ERNIE-Image, Boogu Image, Krea2 (Raw + Turbo), Mage-Flow, SeFi-Image (1B/2B/5B),
HiDream-O1-Image, Ideogram4.

**New edit models:** LongCat Image Edit, Boogu Image Edit, Mage-Flow-Edit.

**New video models:** MiniMax-H3 (day-1 support, 2026-08-04), LTX-2.3 / LTX-2.5,
HunyuanVideo 1.5, LingBot-Video.

**New capabilities (this is the bigger story than the models):**

- **Inference caching** — 6 modes (`ucache`, `easycache`, `dbcache`, `taylorseer`, `cache-dit`,
  `spectrum`). Skips whole forward passes. Free speedup on existing models, no new download.
- **ADetailer** — YOLOv8 detect + cropped inpaint pass. Automatic face/hand repair.
- **Hires fix** — native `sd_hires_params_t` with 10 upscaler modes (latent + model-based).
- **`--auto-fit` VRAM placement** — derives diffusion/te/vae placement automatically from the
  model and available VRAM. Directly serves Blink's non-technical audience.
- **Layer streaming** (`stream_layers` + `max_vram`) — run models larger than VRAM.
- **IP-Adapter** (SD1.5/SDXL incl. Plus), **PuLID** face identity, **textual inversion embeddings**.
- **Native cancellation** (`sd_cancel_generation`) — replaces Blink's between-steps AtomicBool poll.
- **Device enumeration** (`sd_list_devices`) — enables a real GPU picker.
- **Capability probes** (`sd_ctx_supports_image_generation` / `_video_generation`) — validate a
  model before load instead of failing mid-generation.
- **Dynamic ControlNet** (`sd_ctx_load_control_net` / `_unload`) — no full context rebuild.
- **imatrix** quantization, **RPC** distributed inference, **INT8 convrot**.
- **`sd_img_gen_params_to_str`** — generation metadata as a string, useful for gallery records.

## 3. Migration cost — smaller than the commit count suggests

Verified: the header did **not** move (`include/stable-diffusion.h` in both revs), so
`crates/sd-sys/build.rs` bindgen config needs no change.

Blink calls 40 `sd_sys::*` symbols. Only these break:

| Symbol | Change | Blink call site |
|---|---|---|
| `generate_image` | `sd_image_t*` → `bool` (results via out-param) | `ffi_bridge.rs` |
| `generate_video` | `sd_image_t*` → `bool` | `video.rs` |
| `upscale` | `sd_image_t` → `bool` | `upscaler.rs` |
| CPU offload fields | **Removed from `sd_ctx_params_t`** — replaced by `backend` / `params_backend` / `split_mode` strings | `types.rs` `ContextConfig`: `keep_clip_on_cpu`, `keep_vae_on_cpu`, `offload_params_to_cpu` |

Also new: `free_sd_images()` is now the correct way to release result buffers.

Everything else Blink uses (`new_sd_ctx`, `sd_ctx_params_init`, `sd_sample_params_init`,
progress/preview/log callbacks, `preprocess_canny`, sampler + scheduler enums) is unchanged.

**Estimate: 3 call sites + 1 struct rework.** The offload-field removal is the only one needing
design thought, because Blink's SettingsDrawer surfaces those as user-facing toggles — they'd map
onto `params_backend` strings, or be replaced outright by `auto_fit`.

> **Update (migration completed, branch `feat/sdcpp-bump-2026-09`).** The estimate above was
> too low. Building against `6b3edaa` surfaced three more breakages plus two build issues that a
> header-symbol diff could not have predicted:
>
> | Additional breakage | Resolution |
> |---|---|
> | `vae_decode_only` removed from `sd_ctx_params_t` (#1653) | Dropped; sd.cpp always keeps the encoder |
> | `auto_resize_ref_image` + `increase_ref_index` folded into one `ref_image_args` key-value string (#1540) | Dropped — auto-resize is now `resize_before_vae`, already defaulting to `true` (`src/model/diffusion/model.hpp:24`), so behaviour is preserved and sd.cpp applies its per-model ref-image preset |
> | `new_upscaler_ctx` lost `offload_params_to_cpu`, gained `backend` / `params_backend` | Pass null for both (sd.cpp default placement) |
>
> **Build issues (neither is an sd.cpp bug):**
>
> 1. **Stale CMake cache.** New ggml added `if (DEFINED MATH_LIBRARY)` around
>    `target_link_libraries(ggml-base ...)`. `MATH_LIBRARY` is only ever set by
>    `ggml/tests/CMakeLists.txt` (tests are OFF), but it was sitting as a
>    `MATH_LIBRARY-NOTFOUND` entry in all ten cached `target/debug/build/sd-sys-*`
>    CMake caches, so the guard passed and Generate failed. Fixed by deleting those
>    build dirs. **Expect this on any machine with a pre-bump build tree** — a clean
>    checkout is unaffected.
> 2. **`advapi32` now required on Windows.** ggml's
>    `ggml_backend_cpu_device_context` reads CPU topology from the registry
>    (`RegOpenKeyExA` / `RegQueryValueExA` / `RegCloseKey`). Added
>    `cargo:rustc-link-lib=advapi32` to `crates/sd-sys/build.rs`. Note `cargo check`
>    does **not** catch this — it does not link. Only `cargo build` / `cargo test` do.
>
> Also fixed: a **pre-existing** broken unit test (`new_context_with_missing_model_returns_error`)
> that called `unwrap_err()` on `Result<SdCppContext, _>`, which cannot compile because
> `SdCppContext` has no `Debug` impl. `cargo test` was already failing before the bump.
>
> **Verified:** `cargo build --workspace` links, `cargo test --workspace` passes 11/11,
> `cargo clippy` adds no new warnings. **Not yet verified: actual image generation** — the
> migration is compile-verified only, and needs a real generation run per architecture.

## 4. Catalog recommendations

The key packaging insight: **most 2026 models require a large separate LLM text encoder**, which
dominates total download size. For a "type a prompt, get an image" app this matters more than
raw model quality. Grouped by that constraint:

**Light text-encoder path (best fits Blink):**
- **Anima / Anima2** — Qwen3-0.6B-Base encoder. Smallest modern stack found. GGUF available.
  `--cfg-scale 6.0`, euler.
- **Chroma1-Radiance** — needs only diffusion model + t5xxl; pixel-space, no VAE. GGUF available.
  `--cfg-scale 4.0`, euler.
- **HiDream-O1-Image-Dev** — loads as a single `-m` checkpoint, `--cfg-scale 1.0`. Simplest to package.
- **Z-Image Turbo** *(already shipped)* — Qwen3-4B, 8 steps, cfg 1.0. Still a strong default.

**Heavy text-encoder path (Advanced tier only):**
- Krea2 Turbo (Qwen3-VL-4B), Mage-Flow Turbo (Qwen3-VL-4B, 4 steps) — reasonable.
- LongCat / Boogu (Qwen2.5-VL-7B / Qwen3-VL-8B), Ideogram4 (Qwen3-VL-8B **plus** a second
  uncond diffusion model), Lens (**GPT-OSS-20B**) — very large total footprint.

**Note:** PiD is a decoder/upscaler, not a text-to-image model — it runs as an edit pipeline
(512→2048 in 4 steps). Would slot into Blink's *upscale* path, not the model selector.
NVIDIA released it under NSCLv1 (non-commercial) — check `commercial: false` in the manifest.

Not verified here: file sizes, VRAM figures, and output quality for the new models. Those need
measurement before they go in `models.json` with `vram_mb` numbers.

## 4b. Measured: CPU offload is *faster* here — and first-run cost is I/O, not placement

> **Retraction.** An earlier version of this section claimed CPU offload cost 10x. That was a
> measurement error: the two configs were run once each, in sequence, and the first run read
> ~8.5 GB of model files from cold disk while the second inherited them from the OS page cache.
> The difference measured was the file cache, not the offload setting. Corrected below.

Smoke test (`crates/sd-wrapper/tests/smoke_generation.rs`), Z-Image Turbo Q8, 512x512,
4 steps, seed 42, RTX 4070 SUPER (12 GB). All trials warm-cache, alternating order:

| Trial | `offload_params_to_cpu=true` (current) | `=false` |
|---|---|---|
| 1 | 5.97 s | 15.21 s |
| 2 | 6.33 s | 11.51 s |
| 3 | 6.46 s | 15.40 s |
| **Median** | **~6.3 s** | **~15.2 s** |

**CPU offload is ~2.4x faster on this hardware**, and GPU-resident shows noticeably higher
variance (11.5-15.4 s). Blink's current architecture-based rule is doing the right thing here.

Plausible mechanism: with offload off, sd.cpp reports
`total params memory size = 9988.08MB (VRAM)` and the VAE compute buffer alone is 1664 MB —
roughly 11.6 GB against a 12.28 GB card. Keeping every stage resident leaves almost no
headroom. With offload on, each stage is staged to CUDA0 and released, so peak VRAM is far
lower.

**Both placements produce byte-identical output** (FNV-1a over the raw RGBA:
`cb2162f52e872c5b`, 1048576 bytes, both modes). The `params_backend` translation is
numerically transparent — it changes residency and speed, not results.

### The real first-run cost is disk I/O

The discarded cold-cache run is still informative on its own terms: the very first generation
after boot (or after switching models) took **154 s**, dominated by reading 6.27 GB of
diffusion weights plus 2.33 GB of text encoder from disk. Warm, the same config takes ~6 s.

For a "type a prompt, get an image" app this is a real UX problem — a first-time user's
opening generation can take minutes with no obvious explanation. Worth surfacing in the UI
(a distinct "loading model" phase, which the progress callback can already distinguish) rather
than letting it look like a hung generation.

**Implication for `auto_fit`:** the case for it is *not* that current behaviour is slow — it
isn't, on this card. The case is that the rule is hardware-blind: it keys off architecture name
with no knowledge of VRAM, so the same setting that wins on 12 GB may be wrong on a 24 GB card
(where resident would likely win) or insufficient on 8 GB. `auto_fit` decides from measured
VRAM. That is a robustness argument, not a performance one, and it should be validated on more
than one GPU before being treated as a win.

*Caveat: n=3 per config, one machine, one model, one resolution.*

## 4c. LoRA on Z-Image: two separate bugs, one fixable

Investigated on branch `feat/sdcpp-bump-2026-09` with `crates/sd-wrapper/tests/smoke_generation.rs`
(`lora_strength_probe`). Z-Image Turbo Q8, 512x512, 4 steps, seed 42.

### Bug 1: `feed_forward` LoRA tensors silently discarded — FIXED (one line)

sd.cpp converts LoRA key underscores to dots, with a `protected_tokens` allowlist in
`src/name_conversion.cpp`. That list has `cross_attn` and `output_proj` (added by #1786 for
Anima) and `token_refiner` (#1864, MiniMax H3), but **not `feed_forward`**. So
`feed_forward_w1` becomes `feed.forward.w1`, matches no model tensor, and is dropped with a
WARN nobody reads.

Measured impact on real LoRAs:

| LoRA | Key style | FFN tensors dropped |
|---|---|---|
| `ZIMAGE-CCD-V1` | kohya (`lora_down/up`) | 270 / 630 (43%) |
| `pussy-zimage-v1` | diffusers (`lora_A/B`) | 180 / 480 (37.5%) |
| `ohwx ohx_caro` | diffusers, attention-only | 0 (unaffected) |

Adding `"feed_forward",` to `protected_tokens` drops unused-tensor warnings from 810 to 0 and
increases LoRA influence ~2.1-2.7x (divergence at mult 1.0: 12.42 -> 25.57 / 255). Verified on
both key styles. Worth upstreaming — one line, two direct precedents.

Note the inconsistency this caused: an attention-only LoRA was always applied correctly while
LoRAs with FFN layers lost ~40% of their weights. That alone makes LoRA strength feel
unpredictable from file to file.

**How the fix ships.** The submodule stays pinned to a pristine upstream SHA; the fix lives as
`crates/sd-sys/patches/0001-feed_forward-protected-token.patch` and is applied to the submodule
working tree by `crates/sd-sys/build.rs` (`apply_vendored_patches`) on every build. Consequences
to know about:

- **A dirty submodule working tree is the expected, correct state.** ` m crates/sd-sys/stable-diffusion.cpp`
  in `git status` is what "the patch is applied" looks like. The recorded gitlink SHA must not
  change.
- `git submodule update` refuses to discard local changes without `--force`. If it is forced, the
  next `cargo build` re-applies the patch.
- The apply is deliberately plain `git apply` — **not** `--3way`, which implies `--index` (it
  would stage the change inside the submodule) and whose `--check` succeeds in both directions,
  destroying the already-applied test. It also runs with git's configured `core.autocrlf` rather
  than forcing it off: on a CRLF checkout, forcing it off makes every apply fail. `.gitattributes`
  pins `*.patch` to LF, which is the part that does need forcing.
- When upstream merges this one line, the patch stops applying and the build **fails loudly**
  with a message saying upstream merged it and to delete the patch file. Deleting it is the whole
  fix. `crates/sd-sys/tests/vendored_patches.rs` then still asserts the token is present.
- `BLINK_SKIP_SD_PATCHES=1` builds against pristine upstream, with a warning. Expect dropped LoRA
  FFN tensors.

### Bug 2: LoRA applied ~4x too strongly — STILL PRESENT, no fix available

Upstream [issue #1071](https://github.com/leejet/stable-diffusion.cpp/issues/1071),
"LoRA getting loaded four times in a row with z-image-turbo", is marked **Closed** on GitHub
but the behaviour is still present at `6b3edaa`: the probe logs **4 LoRA file loads per
generation** on the runtime path.

Blink compensates in `src-tauri/src/commands/generation.rs` by dividing the user's multiplier
by 4 for z-image. **Keep this.** It is empirically calibrated — at effective 1.0 these LoRAs
overwhelm the prompt and impose their own subject; 0.25 is where they behave.

A caution for anyone re-deriving this: clipping/saturation metrics do *not* detect it. Measured
clipping at effective 1.0 is 0.02%, i.e. no burn at all. The failure mode is the LoRA dominating
the prompt, which needs a human eye or a prompt-adherence metric, not a histogram.

### `lora_apply_mode: Immediately` is NOT a workaround — it destroys quantized models

sd.cpp gained `lora_apply_mode` (AUTO / IMMEDIATELY / AT_RUNTIME) since the old pin. Blink never
set it, so AUTO applies, and AUTO picks AT_RUNTIME whenever weights are quantized or params are
offloaded — both true here. Since AT_RUNTIME is the path that loads 4x, forcing IMMEDIATELY
looked like a candidate fix. It is not:

| multiplier | AT_RUNTIME | IMMEDIATELY |
|---|---|---|
| 0.25 | 10.21 / 255, 0.01% clipped | 54.16 / 255, 2.07% clipped |
| 0.50 | 16.52 / 255, 0.05% clipped | 55.27 / 255, 2.12% clipped |
| 1.00 | 25.57 / 255, 0.02% clipped | 51.35 / 255, 1.61% clipped |

IMMEDIATELY output is **pure checkerboard noise**, not an image. Divergence is also
non-monotonic (0.25 diverges more than 1.0) because the multiplier is irrelevant once the
weights are destroyed. This is the documented "precision and compatibility issues with quantized
parameters" — merging LoRA deltas into Q8 weights corrupts them. AUTO's refusal is correct.

`LoraApplyMode` is now plumbed through `ContextConfig` and defaults to `Auto`; the probe accepts
`BLINK_TEST_LORA_MODE=immediate|runtime|auto`. Do not ship anything but `Auto` for quantized
models.

### Net effect on multiplier calibration

The two bugs push in opposite directions: bug 1 was making LoRAs ~2.4x *weaker*, bug 2 makes
them ~4x *stronger*. Fixing bug 1 while bug 2 remains means the same slider value now produces a
visibly stronger result than before the bump. Existing gallery entries and saved settings will
not reproduce at identical parameters. The 0.25 divisor was re-validated by eye against
post-fix output and still looks right, but it is compensating for bug 2 only.

## 5. Suggested sequencing

1. **Bump the submodule + fix the 4 breakages.** Unlocks everything else. Rebuild CUDA/Vulkan/Metal.
2. **Wire caching (`spectrum` or `cache-dit`).** Biggest user-visible win per unit of work —
   existing models get faster with no new download. Hide behind a single "Fast mode" toggle.
3. **Replace offload toggles with `auto_fit`.** Removes three confusing settings and makes
   placement hardware-aware. Note section 4b: current behaviour is *not* slow on a 12 GB card,
   so this is a robustness/simplicity change, not a measured speedup. Validate on a second GPU
   before shipping.
4. **Expose the 16 missing samplers + scheduler picker** under Advanced. Cheap; two dropdowns.
5. **Native cancel + capability probes.** Correctness/robustness cleanup.
6. **ADetailer as a one-click "Fix faces" button.** Strong fit for non-technical users, but needs
   a YOLOv8 detector shipped in the catalog — note the converter script requires Python, so the
   *converted* `.safetensors` must be hosted, not converted on the user's machine.
7. **Catalog refresh** — add Anima + Chroma1-Radiance to Tier 1 after measuring.
8. **LTX-2.5 / MiniMax-H3** for video, replacing or joining Wan 2.1.

## 6. Watch-outs

- sd.cpp README still warns: *"API and command-line option may change frequently."* Expect this
  again; consider pinning to tagged releases rather than `master`.
- Upstream added its own embedded web UI (2026-04-11). Not a competitor to Blink's native app,
  but it does mean upstream now has opinions about UX defaults worth borrowing.
- Ideogram4 expects a **JSON-structured prompt** for best results — incompatible with Blink's
  single-line prompt bar without work. Deprioritize.
- ADetailer's converter and SeFi's `convert_sefi.py` are Python — fine for you as the packager,
  but must never land in the user install path.

## 4d. Measured: `spectrum` inference caching cannot engage on a 4-step Turbo model

Item 6's measurement gate, run on `adcea9a` (items 1-5 landed). Full numbers and methodology:
`docs/traces/cache-baseline-6b3edaa.txt`.

**The shipping configuration — Z-Image Turbo Q8, 512x512, 4 steps, Euler, cfg 1.0, seed 42,
`auto_fit` placement. One generation per process, `--test-threads=1`, arms alternated, first
run of the session discarded.**

| arm | runs (s) | median (s) | FNV-1a over raw RGBA |
|---|---|---|---|
| `off` | 3.9583 / 3.9140 / 3.9204 | **3.9204** | `cb2162f52e872c5b` |
| `spectrum` | 3.9821 / 3.9365 / 3.9194 | **3.9365** | `cb2162f52e872c5b` |

The speed gate (`median(spectrum) <= 0.85 x median(off)`) fails: spectrum is 0.4% *slower*,
i.e. indistinguishable from noise. The output is **byte-identical**, which the plan's
acceptance criterion 2 correctly treats as proof the cache did nothing rather than as good news.

**Root cause, and it is structural, not a bug.** `sd_cache_params_init` sets
`spectrum_warmup_steps = 4` (`stable-diffusion.cpp:3546`) and `spectrum_stop_percent = 0.9`
(`:3547`). `SpectrumState::should_predict()` (`src/runtime/spectrum.hpp:49-55`) returns false
while `cnt < warmup_steps` and again once `cnt >= stop_step`, where
`stop_step = (int)(0.9 * total_steps)`. On a 4-step generation that is `cnt < 4` and
`cnt >= 3` — the predict path is closed twice over, for every step. **Any model whose step
count is at or below the warmup threshold gets nothing from `spectrum`, by construction.**
That is precisely the Turbo/Lightning/schnell class Blink leads with.

**The mechanism itself works.** Diagnostic arm at 20 steps (not a configuration to ship — 20
steps at cfg 1.0 on a Turbo model — run only to separate "inert here" from "broken"):

| arm | runs (s) | median (s) | FNV-1a |
|---|---|---|---|
| `off` | 7.2302 / 7.2430 / 7.2431 | **7.2430** | `22983f97f82afb53` |
| `spectrum` | 5.3680 / 5.3739 / 5.3950 | **5.3739** | `836a6864b9ba1308` |

Ratio 0.7420 — **25.8% faster**, and the output changes. So on a >=20-step model (Flux dev,
SDXL) the speed gate would pass comfortably.

**Shipped as:** one "Fast mode" toggle in the Settings drawer's existing Performance section,
**off by default**. No cache mode and no `spectrum_*` parameter is exposed. The human-eye
quality comparison the plan makes decisive for a *default-on* flip was **not performed** and is
not needed for an opt-in toggle — but it must be done before anyone considers defaulting it on.

**If this is ever revisited,** the right shape is a per-model default keyed off the step count
(on above ~20 steps, off below), not a global flip — and `spectrum_warmup_steps` is the knob
that decides where that line sits. `spectrum` is also unsupported for CFG++ samplers, which
sd.cpp warns about and silently disables (`stable-diffusion.cpp:2588-2595`).

### Also measured while implementing item 3

Under item 2's `auto_fit` placement sd.cpp chose `--params-backend "diffusion=disk,te=disk,vae=disk"`,
so weights are re-streamed lazily inside **every** generation, not held resident. On a warm
4-step run the first `sampling`-phase progress event arrives at a median of **2.13 s of a
3.24 s run — about 66%**, all of it the lazy read of ~838 tensors (386 text-encoder + 452
diffusion). Warm `SdContext::new` is only **0.26-0.38 s**, because it maps rather than reads.
The **cold** cost of `new_sd_ctx` under `disk` placement remains unmeasured.
