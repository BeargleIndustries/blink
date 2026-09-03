# Blink — Improvement Ideas, September 2026

Grounded in: the sd.cpp bump to `6b3edaa` (what the engine can now do), a survey of the
local-image-gen landscape, and an inventory of what Blink already ships. Every idea is scored
against one question: **does it make "type a prompt, get an image" better without adding a
knob the user has to understand?**

Companion to `upgrade-research-2026-09.md` (engine-level findings) — this file is about product.

---

## 0. Positioning — the gap Blink actually fills

The 2026 landscape sorts cleanly by platform:

| App | Platform | Model | Notes |
|---|---|---|---|
| [Draw Things](https://jonbrown.org/blog/running-image-generation-locally-on-macos-with-draw-things-2026/) | Apple only | Native, free | Best-in-class on Mac. FLUX.2, Wan 2.2, LTX-2.3, on-device LoRA training. 4.5★. |
| [Fooocus](https://locallyuncensored.com/blog/easiest-local-ai-image-generator.html) | Win/Linux, NVIDIA | Python bundle | One-click, but a Python environment under the hood. |
| [DiffusionBee](https://easygoingnerd.com/blog/local-ai-image-generators-beginner/) | Mac (Windows weak) | Native | Dead simple, limited model support. |
| ComfyUI / SwarmUI / Forge | All | Python | Powerful, not simple. |

**Nobody is "Draw Things for Windows and Linux."** Native, no Python, current models, dead
simple. That is Blink's lane, and it is genuinely open. Every idea below should be judged by
whether it strengthens that claim.

Two things Draw Things does that are worth stealing outright: it treats *model loading* as a
first-class visible phase, and it exposes exactly one "quality vs speed" choice rather than a
sampler/scheduler/steps triplet.

---

## 1. Invisible wins — no new UI, users just notice it got better

These are the highest value-per-effort items. None adds a control.

### 1.1 Inference caching → "it's just faster now"
sd.cpp now has six caching modes (`spectrum`, `cache-dit`, `easycache`, …) that skip whole
forward passes. Existing downloaded models get faster with no new download. Enable
`spectrum` (works on both UNet and DiT models) by default; it is a pure speed change with
tunable quality/speed via `spectrum_stop_percent`.

*Simplicity:* zero UI. Optionally a single "Fast mode" toggle under Advanced if any model
shows quality loss. **Do not** expose the six modes or their parameters.

*Effort:* small — `sd_cache_params_t` on `sd_img_gen_params_t`. **Measure before shipping**
(see 4b in the research doc for how easy it is to fool yourself on timing).

> **Measured, not shipped.** At Blink's shipping step count (4, Z-Image Turbo) `spectrum` is
> structurally inert: its warm-up is 4 steps and it stops predicting at 90 % of the run, so on a
> 4-step generation the window is closed twice over (`stable-diffusion.cpp:3546-3547`,
> `spectrum.hpp:49-55`). Output hash identical, no speedup. At 20 steps it works — 25.8 % faster
> with changed output, quality not yet eyeballed. It is also silently disabled for CFG++
> samplers. A "Fast mode" toggle was built, measured, and reverted: a control that does nothing
> on the default model is worse than no control. **The right shape is automatic** — enable only
> when steps exceed the warm-up, and only after a human-eye quality pass on the affected
> catalogue models (SD 1.5 / SDXL at 20–30 steps). See research §4d.

### 1.2 `auto_fit` replaces the architecture-name offload rule
`models.rs` turns on CPU offload for flux/flux-kontext/z-image by *name*, blind to the user's
VRAM. `auto_fit` asks sd.cpp to place diffusion/TE/VAE from measured VRAM. Measured on a
12 GB card the current rule happens to be right (see 4b) — the case for `auto_fit` is that it
will also be right on 8 GB and 24 GB cards, which the name rule cannot be.

*Simplicity:* this **removes** the six raw toggles currently in the Settings drawer
(Offload to CPU, Free Params Early, CLIP on CPU, VAE on CPU, Flash Attention, Memory-Mapped
Loading). Those are sd.cpp internals wearing a user-facing label. `auto_fit` makes them
unnecessary; keep at most one "Low-memory mode" escape hatch.

*Effort:* small. Validate on a second GPU before defaulting.

> **Shipped (`e445b2c`).** The measurement gate passed: `auto_fit` chose disk-backed params for
> every module and was 21 % faster warm than the old rule, with byte-identical output. The six
> toggles are gone; one "Low-memory mode" switch remains under Performance (there is no
> "Advanced" section in `SettingsDrawer.tsx` — the principle in `CLAUDE.md` currently has
> nowhere to put things). Cost of disk placement: ~66 % of each warm generation is weight
> re-streaming — see research §4b addendum. Cold-start under disk placement is unmeasured.
>
> **Corrected after simulating smaller cards (research §4b.2):** below ~9 GB of free VRAM
> `auto_fit`'s plan moves the diffusion *compute* to the CPU — 271 s per image instead of 4 s —
> with no warning. The old architecture-name rule was accidentally right there. Blink now decides
> per load: if the model's file size plus sd.cpp's 2 GB compute reserve will not fit in free VRAM,
> it forces low-memory placement (weights in RAM, compute on GPU, ~8 s) itself, and logs the
> reason. The toggle remains as the user override. Not yet verified on real 8/6/4 GB hardware.

### 1.3 Model-loading is a phase, not a hang
First generation after boot took **154 s** vs ~6 s warm — almost all of it reading 8.5 GB of
weights from disk. To a first-time user that is an app that froze. The progress callback
already distinguishes loading from sampling; show it: *"Loading Z-Image (6.3 GB)… first time
takes a minute"* with a real progress bar, then the step counter.

*Simplicity:* replaces confusion with reassurance. No new control.

> **Shipped (`adcea9a`).** Loading is now a labelled phase; the lazy weight stream that used to
> read as "Step 386 / 386" carries `phase == loading`. Two corrections to the claim above, found in
> execution: (1) the progress callback did *not* already distinguish the phases — the machine had
> to be built (`crates/sd-wrapper/src/progress.rs`), and its rule reads the step index because
> `sample()` emits a `step=0` tick before the lazy load; (2) nothing fires during `load_model`
> itself under `eager_load = false` — the wait is inside the first generation, not the model
> switch (warm `SdContext::new` is 0.26–0.38 s). Native cancel (`0b723c2`) stops sampling
> mid-step but cannot interrupt a weight load; the UI says so.

### 1.4 Native cancel
`sd_cancel_generation()` now exists. Blink's cancel was an `AtomicBool` read once pre-flight —
it did nothing mid-generation.

> **Shipped (`0b723c2`, hardened in `dfaa9f3`).** Cancel now stops sampling inside the current
> step (measured: a 12-step run cancelled at 1.5 s returns ~0.65 s later instead of 3.9 s later)
> and is no longer discarded when issued while queued or during pre-flight. Two corrections to
> the original claim: it **cannot** interrupt a model load — sd.cpp's loader has zero cancel
> checks — and the UI says so honestly; and the first shipped version silently disarmed itself
> after the first model switch (a shared handle cleared by the old context's drop), caught in
> code review and fixed with a regression test that fails against the old code.

### 1.5 Validate before load
`sd_ctx_supports_image_generation()` / `_video_generation()` let Blink refuse a model that
cannot do what the user is about to ask, *before* spending a minute loading it. Pair with the
existing model-import flow: import → probe → tell the user what this model can do.

### 1.6 Ship the `feed_forward` LoRA fix
One line in vendored sd.cpp (`docs/upstream/`). Restores ~40% of the weights in most
Z-Image LoRAs. Zero UI. Users with LoRAs will just find they work properly. Note the
calibration consequence in research doc 4c.

---

## 2. One-button features — each is a single verb the user already understands

The bar: if it needs more than one control, it's not ready for the main UI.

### 2.1 "Edit with words" ★ highest impact
Load a picture, type *"make the jacket red"* or *"remove the person in the background"*, get
the edited picture. No mask, no brush, no inpaint mode. This is the feature that turns a
generator into a tool people open twice.

sd.cpp now supports several instruction-following edit models:
- **Qwen Image Edit 2509 / 2511** — strongest, but needs Qwen2.5-VL-7B encoder (large).
- **Z-Image Omni** — same family as the Z-Image Turbo already shipped; preset `z_image_omni`
  exists in sd.cpp. *Verify weights and licence before catalogue entry.*
- **FLUX.2-klein** — small, has a `flux2` edit preset.
- **Mage-Flow-Edit**, **LongCat Edit**, **Boogu Edit** — heavier encoders.

Blink already has Kontext editing plumbing (`ref_images`), so the engine side is mostly
there; sd.cpp now auto-selects the reference-image preset per model, which removes the
last bit of per-model configuration.

*Simplicity:* a second tab or a drop-target: **Create** / **Edit**. That's the whole UI.
*Effort:* medium — mostly catalogue + UI. The current inpaint/mask UI can move under Advanced.

### 2.2 "Fix faces" (ADetailer)
sd.cpp's ADetailer runs YOLOv8 face detection + a cropped inpaint pass. One button on the
result: *Fix faces*. Every consumer app that has this gets used constantly.

*Catch:* the YOLOv8 detector must be converted with a Python script (`ultralytics` + `torch`).
That is fine for *you* as the packager — host the converted `.safetensors` in the catalogue —
but it must never appear in the user's install path. Ship `face_yolov8n` only; hand detection
can wait.

### 2.3 "Bigger" — hires fix as the upscale path
sd.cpp now has a native hires pipeline (`sd_hires_params_t`, 10 upscaler modes including
model-based). Blink's current Upscale button is bare ESRGAN. Hires fix re-diffuses at the
higher resolution, so it adds real detail rather than sharpening. Same button, better output.

### 2.4 "More like this"
Right-click / long-press a gallery image → four variations. Implementation: same prompt, same
seed ± small offsets, or same seed with `strength 0.6` img2img. Blink has no batch or
variation concept today; this is the least-intimidating way to introduce one. Gallery
currently offers only select and delete.

### 2.5 Style chips
Fooocus's single most-loved feature is a row of style presets ("Photo", "Anime", "Painting",
"Cinematic", "Sketch"). They are just prompt prefixes/suffixes plus a default model hint. Blink
has no presets. Five chips above the prompt bar, one active at a time, is enough. Pairs
naturally with 3.1 (Anima = the "Anime" chip's model).

---

## 3. Catalogue refresh

Principle: **total download size, not model quality, is the constraint for this audience.**
Most 2026 models ship with a large separate LLM encoder that dwarfs the diffusion weights.

### 3.1 Add
| Model | Why | Encoder | Tier |
|---|---|---|---|
| **Anima / Anima2** (2B) | Anime specialist, ~4 GB, runs in [6 GB VRAM](https://vantagewithai.com/anima-preview-2-powerful-anime-ai-model-that-runs-on-just-6gb-vram/). Qwen3-0.6B encoder — smallest modern stack. | 0.6B | 1 |
| **Z-Image Turbo** *(have)* | Still the default. Apache 2.0, sub-second 8-step, [4–6 GB quantised](https://zimage.run/blog/zi-181-z-image-vs-krea-2-en-20260805). | Qwen3-4B | 1 |
| **Krea 2 Turbo** | Wins on [style diversity and identity preservation](https://www.skool.com/myaiforce-5872/krea-2-vs-z-image-anatomy-details-styles-realism-tested); 1K/2K output modes. Licence: free under $1M revenue / 50 seats — fine for Blink's users, note it in `LicenseInfo`. | Qwen3-VL-4B | 2 |
| **FLUX.2-klein** | Small FLUX.2 with an edit mode. | — | 2 |
| **Chroma1-Radiance** | Apache 2.0, real CFG + negative prompts, no VAE needed. Heavy at FP16 but Q4 GGUF is ~5 GB. The "uncensored FLUX" the community asks for. | t5xxl | 2 |
| **Z-Image Omni** or **Qwen-Image-Edit-2509** | For 2.1. Pick one for tier 1 based on total size. | varies | 1–2 |
| **LTX-2.5** | Video, replaces/joins Wan 2.1 1.3B. Has a dedicated scheduler in sd.cpp. | — | 2 |

### 3.2 Retire or relabel
- `metadata.json` has **stale entries** whose files no longer exist on disk (the `flux/`
  directory is gone but `flux-schnell-q4`, `flux-schnell-q8`, `flux-kontext-dev-q4` still show
  `ready`). Reconcile on startup: mark missing files as *not downloaded*, don't trust cached
  status.
- SD 1.5 can drop to an "Classic" section. Anima and Z-Image Turbo both beat it at similar
  VRAM.

### 3.3 Don't add
- **Ideogram4** — wants JSON-structured prompts; fights the prompt bar.
- **Lens** — needs GPT-OSS-20B as text encoder. Absurd download for this audience.
- **PiD** — it's a decoder/upscaler, not a generator; belongs in the "Bigger" path if anywhere.
- **SeFi-Image** — gated on HF and needs Python conversion scripts.

---

## 4. Remove things

Simplicity is subtractive too.

- **Six raw perf toggles** in Settings → replaced by `auto_fit` (1.2).
- **Sampler dropdown** (5 of 21 exposed) → hide entirely. Ship the model's recommended sampler
  and scheduler from the catalogue. Blink is not the app for people who care about
  `dpm++2m_sde_bt`. If a power user wants it, it's one line in Advanced.
- **The LoRA debug logging** currently in the working tree — keep a single line at
  `log::debug!` for "N LoRA tensors applied, M skipped" (that number is what would have found
  the `feed_forward` bug in a minute), drop the rest.

---

## 5. Suggested order

1. **1.6 + 1.2 + 1.3 + 1.4** — one release of "it just works better." Fixes LoRAs, kills six
   toggles, makes first-run honest, makes cancel real. No new features to explain.
2. **1.1 caching** — measured, then on by default.
3. **2.1 Edit with words** — the headline feature. Catalogue one edit model first.
4. **3.1 Anima + 2.5 Style chips** — ship together; the Anime chip is Anima's reason to exist.
5. **2.2 Fix faces, 2.3 Bigger, 2.4 More like this** — polish tier.
6. **3.1 video** last; it's the least "type a prompt, get an image."

---

## 6. Things to keep saying no to

- Node graphs, workflows, custom pipelines.
- Sampler/scheduler/CFG as primary UI.
- On-device LoRA *training* (Draw Things does it; it's a different product).
- Cloud fallback / hosted generation. The whole point is local.
- More than one prompt box.

Sources: [Draw Things 2026](https://jonbrown.org/blog/running-image-generation-locally-on-macos-with-draw-things-2026/), [Draw Things review](https://www.tooljunction.io/ai-tools/draw-things), [easiest local generator 2026](https://locallyuncensored.com/blog/easiest-local-ai-image-generator.html), [local generators beginner guide](https://easygoingnerd.com/blog/local-ai-image-generators-beginner/), [Z-Image vs Krea 2](https://zimage.run/blog/zi-181-z-image-vs-krea-2-en-20260805), [Krea 2 vs Z-Image tested](https://www.skool.com/myaiforce-5872/krea-2-vs-z-image-anatomy-details-styles-realism-tested), [Krea 2 local guide + licence](https://localaimaster.com/blog/krea-2-local-guide), [Anima on 6 GB](https://vantagewithai.com/anima-preview-2-powerful-anime-ai-model-that-runs-on-just-6gb-vram/), [Anima review](https://diffusiondoodles.substack.com/p/anima-light-fast-and-slightly-unruly), [Chroma local guide](https://localaimaster.com/blog/chroma-local-guide), [best local models 2026](https://localaimaster.com/blog/best-local-image-models-compared), [open-source model families 2026](https://magichour.ai/blog/open-source-image-generation-models).

## In-app verification, 2026-09-03

Driven through the real Tauri window (WebView2 remote debugging on :9222, not the
Vite page), Z-Image Turbo Q8 on the 12 GB RTX 4070 SUPER. What the library tests
had not caught:

- **Cancel button was inert.** `PromptBar` bound `onClick={generating ? onCancel :
  handleGenerate}`; Solid evaluates that once at render, so the button always called
  the generate handler. The native cancel itself worked (a direct `cancel_generation`
  invoke stopped sd.cpp in ~3.6 s). Fixed; the button now stops a 20-step run inside
  the current step and the UI is idle 0.1 s after the click.
- **Previews stalled sampling 13 s each.** Blink asked sd.cpp for `PREVIEW_TAE`, but the
  only TAESD on disk is the SDXL one, so for Z-Image sd.cpp fell back to the full VAE.
  Under sampling-time VRAM pressure that decode took 13 s every third step: a 20-step
  image took 134 s instead of 14 s, and cancel could not land until the decode finished.
  Now `PREVIEW_PROJ` (latent projection, effectively free) unless a TAESD is loaded.
- **Previews never reached the UI at all.** The preview closure JPEG-encoded an RGBA
  buffer; `image` 0.25's JPEG encoder only accepts L8/Rgb8, so every frame failed
  silently. Converted to RGB; frames now arrive (64x128 for a 512x1024 projection).
- **`placement:` line never printed.** It used `log::info!` and the app installs no
  logger. Now `eprintln!` like the other `[blink]` lines: observed
  `placement: auto_fit (10361 MiB usable >= 8320 MiB needed)`.

- **Preview thumbnail and blank Size dropdown.** The projection preview rendered at its 64x128 intrinsic size; it now fills the canvas. The Size select had no entry for Z-Image's 512x1024 default so it showed blank; the list now includes the 1:2 pairs and falls back to an entry for any saved size. Along the way: Solid hoists literal keys of a JSX style object into the static template and applies the spread at runtime, so `style={{ ...inputStyle, width }}` never overrode the shared 80px. Merged at module level instead (four sites in the drawer).

Checked and passing as shipped: the Settings drawer has one placement control
("Low-memory mode") and none of the six old toggles; the progress UI shows "Loading"
before step 1 and never the word "tensor"; a 20-step generation is 14.4 s end to end.
