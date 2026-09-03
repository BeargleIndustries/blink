//! End-to-end smoke test for the sd.cpp FFI migration.
//!
//! The generation test needs a real model on disk and a GPU, so it is marked
//! `#[ignore]`. Run it explicitly:
//!
//! ```text
//! cargo test -p sd-wrapper --test smoke_generation -- --ignored --nocapture
//! ```
//!
//! Model paths default to Blink's app-data directory and can be overridden with
//! `BLINK_TEST_MODEL_DIR`.
//!
//! # Do not read timings from a default run
//!
//! These tests are for correctness. Two things make their wall-clock times useless as
//! benchmarks, and both have already produced wrong conclusions once:
//!
//! 1. **Cargo runs tests in parallel.** Both GPU tests contend for the same device, which
//!    inflated one generation from ~6 s to ~77 s. Use `--test-threads=1` when timing.
//! 2. **The OS page cache dominates.** The first generation after boot reads ~8.5 GB of
//!    weights from disk (~154 s observed); every run after that is warm (~6 s). Comparing
//!    a cold run against a warm one measures the file cache, not whatever you changed.
//!
//! To compare configurations, run each one repeatedly and alternating, with
//! `--test-threads=1`, discarding the first run of the session.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Output sanity checking
// ---------------------------------------------------------------------------

/// Why a generated image looks degenerate, or `None` if it looks like a real image.
///
/// This guards the specific failure modes this codebase has actually hit: an all-black
/// frame (the VAE decoding on the wrong backend) and a flat/uniform frame (weights not
/// applied). A migration that compiles but silently produces black output must fail here.
fn degeneracy_reason(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    let expected = (width as usize) * (height as usize) * 4;
    if rgba.len() != expected {
        return Some(format!(
            "buffer is {} bytes, expected {} for {}x{} RGBA",
            rgba.len(),
            expected,
            width,
            height
        ));
    }
    if rgba.is_empty() {
        return Some("buffer is empty".to_string());
    }

    // Look at colour channels only; a fully opaque alpha channel is expected and
    // would otherwise mask a black image as "has variation".
    let colour: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();

    let nonzero = colour.iter().filter(|&&b| b != 0).count();
    if nonzero == 0 {
        return Some("image is entirely black (all colour channels zero)".to_string());
    }

    let mut seen = [false; 256];
    for &b in &colour {
        seen[b as usize] = true;
    }
    let distinct = seen.iter().filter(|&&s| s).count();
    if distinct < 8 {
        return Some(format!(
            "image has only {} distinct colour values — looks flat, not a render",
            distinct
        ));
    }

    let mean = colour.iter().map(|&b| b as f64).sum::<f64>() / colour.len() as f64;
    let variance =
        colour.iter().map(|&b| (b as f64 - mean).powi(2)).sum::<f64>() / colour.len() as f64;
    let stddev = variance.sqrt();
    if stddev < 3.0 {
        return Some(format!(
            "image standard deviation is {:.2} — nearly uniform, not a render",
            stddev
        ));
    }

    None
}

#[cfg(test)]
mod degeneracy_tests {
    use super::degeneracy_reason;

    /// 2x2 RGBA buffer built from a per-pixel colour function.
    fn buf(f: impl Fn(usize) -> [u8; 3]) -> Vec<u8> {
        (0..4)
            .flat_map(|i| {
                let [r, g, b] = f(i);
                [r, g, b, 255]
            })
            .collect()
    }

    #[test]
    fn all_black_is_degenerate() {
        let img = buf(|_| [0, 0, 0]);
        let reason = degeneracy_reason(&img, 2, 2).expect("all-black must be flagged");
        assert!(reason.contains("black"), "unexpected reason: {reason}");
    }

    #[test]
    fn opaque_alpha_does_not_mask_a_black_image() {
        // Alpha is 255 everywhere; only colour channels are zero.
        let img = buf(|_| [0, 0, 0]);
        assert!(
            degeneracy_reason(&img, 2, 2).is_some(),
            "a black image with opaque alpha must still be flagged"
        );
    }

    #[test]
    fn uniform_grey_is_degenerate() {
        let img = buf(|_| [128, 128, 128]);
        let reason = degeneracy_reason(&img, 2, 2).expect("uniform image must be flagged");
        assert!(
            reason.contains("flat") || reason.contains("uniform"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn wrong_buffer_length_is_reported() {
        let reason = degeneracy_reason(&[0, 0, 0, 255], 2, 2).expect("size mismatch must be flagged");
        assert!(reason.contains("expected"), "unexpected reason: {reason}");
    }

    #[test]
    fn varied_image_is_accepted() {
        // Spread values widely so both the distinct-count and stddev checks pass.
        let img: Vec<u8> = (0..64u32)
            .flat_map(|i| {
                let v = (i * 4) as u8;
                [v, 255 - v, v.wrapping_mul(3), 255]
            })
            .collect();
        assert_eq!(
            degeneracy_reason(&img, 8, 8),
            None,
            "a varied image must not be flagged"
        );
    }
}

// ---------------------------------------------------------------------------
// Real generation smoke test
// ---------------------------------------------------------------------------

fn model_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BLINK_TEST_MODEL_DIR") {
        return PathBuf::from(dir);
    }
    let appdata = std::env::var("APPDATA").expect("APPDATA not set");
    PathBuf::from(appdata).join("com.beargle.blink").join("models")
}

/// The parameter placement a timing run is exercising.
struct Placement {
    label: &'static str,
    auto_fit: bool,
    low_memory: bool,
}

/// Read `BLINK_TEST_PLACEMENT`: `auto_fit` (default) | `low_memory` | `resident`.
///
/// This is the knob the item-2 measurement gate alternates between. `resident`
/// means neither flag, i.e. sd.cpp's own default placement.
fn placement_from_env() -> Placement {
    match std::env::var("BLINK_TEST_PLACEMENT").as_deref() {
        Ok("low_memory") => Placement { label: "low_memory", auto_fit: false, low_memory: true },
        Ok("resident") => Placement { label: "resident", auto_fit: false, low_memory: false },
        Ok("auto_fit") | Err(_) => Placement { label: "auto_fit", auto_fit: true, low_memory: false },
        Ok(other) => panic!(
            "BLINK_TEST_PLACEMENT={other:?} is not one of auto_fit | low_memory | resident"
        ),
    }
}

/// Generate a real image through the migrated FFI path.
///
/// Z-Image is deliberately chosen: it is the stack Blink used to force onto the CPU
/// with an architecture-name rule, so it is the one whose parameter placement the
/// move to sd.cpp's `auto_fit` actually changes.
///
/// `BLINK_TEST_PLACEMENT` selects the placement under test:
/// `auto_fit` (default) — sd.cpp decides from measured VRAM;
/// `low_memory` — Blink's pre-auto_fit behaviour, `params_backend=cpu`;
/// `resident` — neither, i.e. sd.cpp's own default placement.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn zimage_txt2img_produces_a_real_image() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, SampleMethod, SdContext};

    let dir = model_dir();
    let diffusion = dir.join("z-image").join("z_image_turbo-Q8_0.gguf");
    let llm = dir.join("z-image").join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    let vae = dir.join("z-image").join("ae.safetensors");

    for (label, path) in [("diffusion", &diffusion), ("llm", &llm), ("vae", &vae)] {
        assert!(
            path.exists(),
            "missing {label} model at {}. Set BLINK_TEST_MODEL_DIR to override.",
            path.display()
        );
    }

    let placement = placement_from_env();
    let config = ContextConfig {
        model_path: None,
        vae_path: Some(vae.to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(diffusion.to_string_lossy().into_owned()),
        llm_path: Some(llm.to_string_lossy().into_owned()),
        n_threads: 4,
        auto_fit: placement.auto_fit,
        low_memory: placement.low_memory,
        control_net_path: None,
        taesd_path: None,
        lora_apply_mode: LoraApplyMode::Auto,
    };

    eprintln!(
        "[smoke] placement={} (auto_fit={}, low_memory={})",
        placement.label, config.auto_fit, config.low_memory
    );

    let started = std::time::Instant::now();
    let ctx = SdContext::new(config).expect("failed to create sd.cpp context");
    eprintln!("[smoke] context loaded in {:?}", started.elapsed());

    // Z-Image Turbo: cfg 1.0, few steps, euler (per sd.cpp docs/z_image.md).
    let params = GenerationParams {
        prompt: "a red apple on a wooden table, studio lighting".to_string(),
        width: 512,
        height: 512,
        steps: 4,
        cfg_scale: 1.0,
        seed: 42,
        sample_method: SampleMethod::Euler,
        ..Default::default()
    };

    let gen_started = std::time::Instant::now();
    let image = ctx
        .txt2img(params, Vec::new(), None, None)
        .expect("txt2img failed");
    eprintln!("[smoke] generated in {:?}", gen_started.elapsed());

    assert_eq!(image.width, 512, "unexpected output width");
    assert_eq!(image.height, 512, "unexpected output height");

    if let Some(reason) = degeneracy_reason(&image.data, image.width, image.height) {
        panic!("generated image failed sanity check: {reason}");
    }

    // Write it out so the result can be eyeballed.
    // Name the file after the placement mode so successive runs do not overwrite
    // each other — the outputs need to be comparable side by side.
    let suffix = placement.label;

    // Checksum the raw RGBA (not the PNG) so the comparison is not affected by
    // encoder settings or metadata.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a
    for &b in &image.data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    eprintln!("[smoke] rgba_fnv1a={hash:016x} bytes={}", image.data.len());

    let out = std::env::temp_dir().join(format!("blink_smoke_zimage_{suffix}.png"));
    image::save_buffer(
        &out,
        &image.data,
        image.width,
        image.height,
        image::ColorType::Rgba8,
    )
    .expect("failed to write smoke-test image");
    eprintln!("[smoke] wrote {}", out.display());
}

/// Two different seeds must produce two different images.
///
/// This is the counterpart to the fixed-seed test above. That one proves determinism;
/// this one proves *liveness* — that generation is not handing back a cached or stale
/// buffer. It matters specifically because the sd.cpp bump changed how result memory is
/// allocated and released (`free_sd_images` replacing manual `libc::free`), and a
/// use-after-free or a reused buffer could easily still produce a valid-looking image.
///
/// A fixed-seed test cannot catch that failure mode; identical output is its pass
/// condition.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn different_seeds_produce_different_images() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, SampleMethod, SdContext};

    let dir = model_dir();
    let diffusion = dir.join("z-image").join("z_image_turbo-Q8_0.gguf");
    let llm = dir.join("z-image").join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    let vae = dir.join("z-image").join("ae.safetensors");
    if !diffusion.exists() || !llm.exists() || !vae.exists() {
        panic!("Z-Image model files missing; set BLINK_TEST_MODEL_DIR");
    }

    let config = ContextConfig {
        model_path: None,
        vae_path: Some(vae.to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(diffusion.to_string_lossy().into_owned()),
        llm_path: Some(llm.to_string_lossy().into_owned()),
        n_threads: 4,
        auto_fit: true,
        low_memory: false,
        control_net_path: None,
        taesd_path: None,
        lora_apply_mode: LoraApplyMode::Auto,
    };

    let ctx = SdContext::new(config).expect("failed to create sd.cpp context");

    let gen = |seed: i64| {
        let params = GenerationParams {
            prompt: "a red apple on a wooden table, studio lighting".to_string(),
            width: 512,
            height: 512,
            steps: 4,
            cfg_scale: 1.0,
            seed,
            sample_method: SampleMethod::Euler,
            ..Default::default()
        };
        ctx.txt2img(params, Vec::new(), None, None)
            .unwrap_or_else(|e| panic!("txt2img failed for seed {seed}: {e:?}"))
    };

    let a = gen(42);
    let b = gen(1337);

    // Both must independently be real images, not just different from each other.
    for (seed, img) in [(42, &a), (1337, &b)] {
        if let Some(reason) = degeneracy_reason(&img.data, img.width, img.height) {
            panic!("seed {seed} produced a degenerate image: {reason}");
        }
    }

    assert_ne!(
        a.data, b.data,
        "seeds 42 and 1337 produced identical pixel data — generation is returning a \
         stale or cached buffer rather than rendering"
    );

    let differing = a
        .data
        .iter()
        .zip(b.data.iter())
        .filter(|(x, y)| x != y)
        .count();
    let frac = differing as f64 / a.data.len() as f64;
    eprintln!("[smoke] seeds 42 vs 1337 differ in {:.1}% of bytes", frac * 100.0);
    assert!(
        frac > 0.10,
        "only {:.2}% of bytes differ between seeds — suspiciously similar",
        frac * 100.0
    );
}

// ---------------------------------------------------------------------------
// LoRA strength probe
// ---------------------------------------------------------------------------

/// Fraction of colour bytes clipped to 0 or 255 — an "overcooked" signal.
///
/// A LoRA applied several times stronger than intended typically saturates: blown
/// highlights and crushed blacks. This gives a number to compare instead of an opinion.
fn clipped_fraction(rgba: &[u8]) -> f64 {
    let colour: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let clipped = colour.iter().filter(|&&b| b == 0 || b == 255).count();
    clipped as f64 / colour.len() as f64
}

/// Mean absolute per-byte difference between two images, 0-255.
fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f64)
        .sum::<f64>()
        / a.len() as f64
}

/// Measure how strongly a LoRA is actually applied, to decide whether the
/// "Z-Image applies LoRA 4x" workaround in `src-tauri/src/commands/generation.rs`
/// is still needed after the sd.cpp bump.
///
/// Renders the same seed with no LoRA, then at several multipliers, and reports
/// divergence from baseline plus a clipping ratio. Run with `--nocapture` and read
/// the table; sd.cpp's own log lines about applied/unmatched tensors are the other
/// half of the evidence.
///
/// Point it at a LoRA with `BLINK_TEST_LORA=<path>`.
#[test]
#[ignore = "diagnostic probe; needs a Z-Image model and a LoRA"]
fn lora_strength_probe() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, LoraConfig, SampleMethod, SdContext};

    let lora_path = match std::env::var("BLINK_TEST_LORA") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[probe] set BLINK_TEST_LORA=<path to .safetensors> to run");
            return;
        }
    };
    assert!(
        std::path::Path::new(&lora_path).exists(),
        "LoRA not found: {lora_path}"
    );

    let dir = model_dir();
    let config = ContextConfig {
        model_path: None,
        vae_path: Some(dir.join("z-image").join("ae.safetensors").to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(
            dir.join("z-image").join("z_image_turbo-Q8_0.gguf").to_string_lossy().into_owned(),
        ),
        llm_path: Some(
            dir.join("z-image")
                .join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf")
                .to_string_lossy()
                .into_owned(),
        ),
        n_threads: 4,
        auto_fit: true,
        low_memory: false,
        control_net_path: None,
        taesd_path: None,
        // BLINK_TEST_LORA_MODE=immediate|runtime|auto (default auto)
        lora_apply_mode: match std::env::var("BLINK_TEST_LORA_MODE").as_deref() {
            Ok("immediate") => LoraApplyMode::Immediately,
            Ok("runtime") => LoraApplyMode::AtRuntime,
            _ => LoraApplyMode::Auto,
        },
    };
    eprintln!("[probe] lora_apply_mode={:?}", config.lora_apply_mode);

    let ctx = SdContext::new(config).expect("context creation failed");

    let render = |mult: Option<f32>| {
        let loras = match mult {
            Some(m) => vec![LoraConfig {
                path: lora_path.clone(),
                multiplier: m,
                is_high_noise: false,
            }],
            None => Vec::new(),
        };
        let params = GenerationParams {
            prompt: "a portrait photograph of a woman, natural light".to_string(),
            width: 512,
            height: 512,
            steps: 4,
            cfg_scale: 1.0,
            seed: 42,
            sample_method: SampleMethod::Euler,
            loras,
            ..Default::default()
        };
        ctx.txt2img(params, Vec::new(), None, None)
            .unwrap_or_else(|e| panic!("txt2img failed at {mult:?}: {e:?}"))
    };

    let base = render(None);
    eprintln!(
        "[probe] baseline (no lora): clipped={:.2}%",
        clipped_fraction(&base.data) * 100.0
    );

    for mult in [0.25f32, 0.5, 1.0] {
        let img = render(Some(mult));
        let diff = mean_abs_diff(&base.data, &img.data);
        let clip = clipped_fraction(&img.data) * 100.0;
        let same = img.data == base.data;
        eprintln!(
            "[probe] mult={mult:<5} diff_from_base={diff:6.2}/255  clipped={clip:5.2}%  \
             identical_to_base={same}"
        );
        let out = std::env::temp_dir().join(format!("blink_lora_probe_{mult}.png"));
        image::save_buffer(&out, &img.data, img.width, img.height, image::ColorType::Rgba8)
            .expect("save failed");
    }
    let out = std::env::temp_dir().join("blink_lora_probe_base.png");
    image::save_buffer(&out, &base.data, base.width, base.height, image::ColorType::Rgba8)
        .expect("save failed");
    eprintln!("[probe] images written to {}", std::env::temp_dir().display());
}

// ---------------------------------------------------------------------------
// Cancellation (item 4)
// ---------------------------------------------------------------------------
//
// These four tests cover the two windows in which Blink used to discard a cancel
// and hand the user a picture they had asked not to have:
//
//   * between `txt2img()` sending the command and the inference thread dequeuing
//     it — erased by the old `thread_cancel.store(false, …)` on dequeue;
//   * between the pre-flight check at the top of `SdCppContext::generate` and the
//     `generate_image` call — erased by sd.cpp's own `reset_cancel_flag()`
//     (`stable-diffusion.cpp:5644`).
//
// The second window is widened deterministically by `BLINK_TEST_PREFLIGHT_DELAY_MS`
// (a `#[cfg(debug_assertions)]` hook in `ffi_bridge.rs`), so none of these is a race.
// Run them with `--test-threads=1`: they set and clear that variable, and the
// process-wide environment is not test-local.

/// The reference Z-Image Turbo Q8 stack, with `auto_fit` placement (item 2's default).
fn zimage_config() -> sd_wrapper::ContextConfig {
    use sd_wrapper::{ContextConfig, LoraApplyMode};

    let dir = model_dir();
    let diffusion = dir.join("z-image").join("z_image_turbo-Q8_0.gguf");
    let llm = dir.join("z-image").join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    let vae = dir.join("z-image").join("ae.safetensors");

    for (label, path) in [("diffusion", &diffusion), ("llm", &llm), ("vae", &vae)] {
        assert!(
            path.exists(),
            "missing {label} model at {}. Set BLINK_TEST_MODEL_DIR to override.",
            path.display()
        );
    }

    ContextConfig {
        model_path: None,
        vae_path: Some(vae.to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(diffusion.to_string_lossy().into_owned()),
        llm_path: Some(llm.to_string_lossy().into_owned()),
        n_threads: 4,
        auto_fit: true,
        low_memory: false,
        control_net_path: None,
        taesd_path: None,
        lora_apply_mode: LoraApplyMode::Auto,
    }
}

fn zimage_cancel_context() -> sd_wrapper::SdContext {
    sd_wrapper::SdContext::new(zimage_config()).expect("failed to create sd.cpp context")
}

fn zimage_cancel_params(steps: u32, seed: i64) -> sd_wrapper::GenerationParams {
    use sd_wrapper::{GenerationParams, SampleMethod};
    GenerationParams {
        prompt: "a red apple on a wooden table, studio lighting".to_string(),
        width: 512,
        height: 512,
        steps,
        cfg_scale: 1.0,
        seed,
        sample_method: SampleMethod::Euler,
        ..Default::default()
    }
}

/// A cancel that arrives while a command is still sitting in the queue must
/// survive until the inference thread dequeues it.
///
/// This is the regression test for the removal of `thread_cancel.store(false, …)`
/// at the three dequeue sites in `context.rs`. It is made deterministic rather
/// than raced: generation 1 is parked inside the widened pre-flight window, so
/// generation 2 is provably still queued when the cancel is issued.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn cancel_before_dequeue_produces_no_image() {
    use sd_wrapper::SdError;

    std::env::set_var("BLINK_TEST_PREFLIGHT_DELAY_MS", "5000");
    let ctx = zimage_cancel_context();
    let ctx = &ctx;

    let first = zimage_cancel_params(4, 42);
    let second = zimage_cancel_params(4, 43);

    let (r1, r2) = std::thread::scope(|s| {
        let a = s.spawn(move || ctx.txt2img(first, Vec::new(), None, None));
        // Generation 1 is now past its pre-flight check and inside the 5 s pause.
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let b = s.spawn(move || ctx.txt2img(second, Vec::new(), None, None));
        // Generation 2's command has been sent and cannot be dequeued until
        // generation 1 returns, so the cancel below lands while it is queued.
        std::thread::sleep(std::time::Duration::from_millis(1000));
        ctx.cancel();
        (a.join().unwrap(), b.join().unwrap())
    });
    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");

    assert!(
        matches!(r2, Err(SdError::Cancelled)),
        "a cancel issued while the command was queued must survive to the \
         pre-flight check, got: {r2:?}"
    );
    assert!(
        matches!(r1, Err(SdError::Cancelled)),
        "the in-flight generation must also report Cancelled, got: {r1:?}"
    );
}

/// A cancel that arrives after the pre-flight check but before `generate_image`
/// must not be swallowed by sd.cpp's `reset_cancel_flag()` at `:5644`.
///
/// This is the regression test for the second `cancel_flag.load()` immediately
/// before the `generate_image` call.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn cancel_during_preflight_window_produces_no_image() {
    use sd_wrapper::SdError;

    std::env::set_var("BLINK_TEST_PREFLIGHT_DELAY_MS", "4000");
    let ctx = zimage_cancel_context();
    let ctx = &ctx;

    let result = std::thread::scope(|s| {
        s.spawn(move || {
            // The pre-flight check runs within microseconds of the send, so by
            // now the generation is provably inside the widened window.
            std::thread::sleep(std::time::Duration::from_millis(1000));
            ctx.cancel();
        });
        ctx.txt2img(zimage_cancel_params(4, 42), Vec::new(), None, None)
    });
    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");

    assert!(
        matches!(result, Err(SdError::Cancelled)),
        "a cancel inside the pre-flight window must yield Cancelled and no image, \
         got: {result:?}"
    );
}

/// Native cancellation stops sampling inside a step instead of at the end of the
/// generation.
///
/// The uncancelled run is measured first, in the same process and on the same
/// context, so the comparison is warm-against-warm.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn native_cancel_stops_mid_generation() {
    use sd_wrapper::SdError;

    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");
    let ctx = zimage_cancel_context();
    let ctx = &ctx;

    // Enough steps that "stops mid-generation" is measurable rather than noise.
    const STEPS: u32 = 12;

    let t0 = std::time::Instant::now();
    ctx.txt2img(zimage_cancel_params(STEPS, 42), Vec::new(), None, None)
        .expect("baseline generation failed");
    let uncancelled = t0.elapsed();
    eprintln!("[cancel] uncancelled {STEPS}-step run: {uncancelled:?}");

    let t1 = std::time::Instant::now();
    let result = std::thread::scope(|s| {
        s.spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            ctx.cancel();
        });
        ctx.txt2img(zimage_cancel_params(STEPS, 43), Vec::new(), None, None)
    });
    let cancelled = t1.elapsed();
    eprintln!("[cancel] cancelled run returned after {cancelled:?}");

    assert!(
        matches!(result, Err(SdError::Cancelled)),
        "cancel during sampling must yield Cancelled, got: {result:?}"
    );
    assert!(
        cancelled.as_secs_f64() < 0.75 * uncancelled.as_secs_f64(),
        "cancel did not take effect mid-generation: cancelled={cancelled:?} vs \
         uncancelled={uncancelled:?}"
    );
}

/// A generation started after a cancelled one completes normally.
///
/// Self-contained on purpose: it issues its own cancel rather than relying on a
/// preceding test having left the flag set, so it keeps working under `--exact`,
/// a rename, or a solo run. It is the guard for the caller-side
/// `cancel_flag.store(false, …)` at the top of `SdContext::txt2img` — without
/// that, the pre-flight check kills this generation immediately.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn generation_after_cancel_succeeds() {
    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");
    let ctx = zimage_cancel_context();

    // Latch both halves of the cancel with nothing in flight.
    ctx.cancel();

    let image = ctx
        .txt2img(zimage_cancel_params(4, 42), Vec::new(), None, None)
        .expect("a generation started after a cancel must succeed");

    assert_eq!(image.width, 512);
    assert_eq!(image.height, 512);
    if let Some(reason) = degeneracy_reason(&image.data, image.width, image.height) {
        panic!("generation after cancel produced a degenerate image: {reason}");
    }
}

/// Cancelling and immediately switching models must not crash.
///
/// This is the use-after-free guard: `SdCppContext::drop` calls
/// `CancelHandle::clear()` before `free_sd_ctx`, and `clear()` cannot take the
/// mutex while a `cancel()` is inside the FFI call, so sd.cpp is never handed a
/// freed context. The test drives that race directly — a thread hammers `cancel()`
/// on the handle while the context is torn down — and then proves the engine is
/// still usable by loading a second context and generating.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn cancel_during_model_switch_does_not_crash() {
    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");

    let first = zimage_cancel_context();
    let handle = first.cancel_handle();

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_writer = std::sync::Arc::clone(&stop);
    let hammer = std::thread::spawn(move || {
        let mut issued = 0u32;
        while !stop_writer.load(std::sync::atomic::Ordering::SeqCst) {
            handle.cancel();
            issued += 1;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        issued
    });

    // Tear the context down underneath the cancels — this is the "switch models"
    // half of the scenario.
    std::thread::sleep(std::time::Duration::from_millis(50));
    drop(first);
    std::thread::sleep(std::time::Duration::from_millis(50));
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let issued = hammer.join().expect("the cancel thread panicked");
    eprintln!("[cancel] issued {issued} cancels across the teardown");
    assert!(issued > 1, "the cancel thread did not actually race the teardown");

    // The replacement context gets its own handle, so the old pending cancel
    // cannot leak into it.
    let second = zimage_cancel_context();
    let image = second
        .txt2img(zimage_cancel_params(4, 42), Vec::new(), None, None)
        .expect("generation after a cancelled model switch must succeed");
    if let Some(reason) = degeneracy_reason(&image.data, image.width, image.height) {
        panic!("post-switch generation produced a degenerate image: {reason}");
    }
}

/// Native cancellation must still work after a model switch.
///
/// `AppState` shares ONE `CancelHandle` for the life of the process and
/// `load_model` builds the replacement context *before* dropping the old one
/// (`state.rs`), so the old context's `Drop` runs last and gets the final word
/// on the handle. An unconditional `clear()` there stores `None` over the live
/// pointer and every later `cancel()` silently degrades to "park it in
/// `pending`" — the user waits out the whole generation. This test drives that
/// exact ordering through the shared handle, so it fails against an
/// unconditional clear and passes against a compare-and-clear.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn cancel_after_model_switch_stops_mid_generation() {
    use sd_wrapper::{CancelHandle, SdContext, SdError};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");

    // The app's sharing shape: one flag and one handle, reused across loads.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_handle = Arc::new(CancelHandle::new());

    let first = SdContext::with_cancel_flag(
        zimage_config(),
        Arc::clone(&cancel_flag),
        Arc::clone(&cancel_handle),
        None,
    )
    .expect("failed to create the first sd.cpp context");
    let second = SdContext::with_cancel_flag(
        zimage_config(),
        Arc::clone(&cancel_flag),
        Arc::clone(&cancel_handle),
        None,
    )
    .expect("failed to create the second sd.cpp context");
    // `load_model`'s order: the old context is released only once the new one
    // exists, so its `Drop` sees a handle already pointing at the new context.
    drop(first);

    // Enough steps that "stops mid-generation" is measurable rather than noise.
    const STEPS: u32 = 12;
    let ctx = &second;

    let t0 = std::time::Instant::now();
    ctx.txt2img(zimage_cancel_params(STEPS, 42), Vec::new(), None, None)
        .expect("baseline generation on the post-switch context failed");
    let uncancelled = t0.elapsed();
    eprintln!("[cancel] post-switch uncancelled {STEPS}-step run: {uncancelled:?}");

    let t1 = std::time::Instant::now();
    let result = std::thread::scope(|s| {
        s.spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            ctx.cancel();
        });
        ctx.txt2img(zimage_cancel_params(STEPS, 43), Vec::new(), None, None)
    });
    let cancelled = t1.elapsed();
    eprintln!("[cancel] post-switch cancelled run returned after {cancelled:?}");

    // Timing first: this is the assertion that discriminates a live native
    // cancel from one that was silently disarmed by the model switch.
    assert!(
        cancelled.as_secs_f64() < 0.75 * uncancelled.as_secs_f64(),
        "native cancel was disarmed by the model switch: cancelled={cancelled:?} vs \
         uncancelled={uncancelled:?}"
    );
    assert!(
        matches!(result, Err(SdError::Cancelled)),
        "cancel during sampling must yield Cancelled, got: {result:?}"
    );
}

/// An undecodable inpainting mask must fail cleanly and leave the context usable.
///
/// The mask decode used to sit *after* the global progress and preview callbacks
/// were installed, so its `?` returned past the teardown: both trampoline boxes
/// leaked and sd.cpp kept calling into them. The decode now runs before the
/// install point. The user-visible half of that is the second generation below.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn an_undecodable_mask_fails_cleanly_and_leaves_the_context_usable() {
    use sd_wrapper::{Img2ImgParams, SdError};

    std::env::remove_var("BLINK_TEST_PREFLIGHT_DELAY_MS");
    let ctx = zimage_cancel_context();

    // A valid PNG for the init image, so the mask is the only thing that fails.
    let buf = image::RgbImage::from_fn(256, 256, |x, y| image::Rgb([x as u8, y as u8, 128]));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(buf)
        .write_to(&mut png, image::ImageFormat::Png)
        .expect("failed to encode the test init image");

    let err = ctx
        .img2img(
            png.into_inner(),
            Some(b"not an image".to_vec()),
            Img2ImgParams {
                base: zimage_cancel_params(4, 42),
                strength: 0.4,
            },
            None,
            None,
            None,
            None,
        )
        .expect_err("an undecodable mask must be rejected");
    assert!(
        matches!(err, SdError::InvalidParams { .. }),
        "expected InvalidParams for an undecodable mask, got: {err:?}"
    );

    let image = ctx
        .txt2img(zimage_cancel_params(4, 43), Vec::new(), None, None)
        .expect("a generation after a rejected mask must succeed");
    if let Some(reason) = degeneracy_reason(&image.data, image.width, image.height) {
        panic!("post-failure generation produced a degenerate image: {reason}");
    }
}

// ---------------------------------------------------------------------------
// Load phase (item 3)
// ---------------------------------------------------------------------------

/// C2 clause (b) — the load-window progress callback is installed and torn down.
///
/// **Zero events fire here today, and that is the measured truth.** `eager_load`
/// defaults to `false` (`stable-diffusion.cpp:252` — "Load all params into the
/// params backend at model-load time **instead of lazily on first use**"), so
/// `new_sd_ctx` maps and measures but streams no tensors and never calls
/// `pretty_bytes_progress`. Confirmed twice: by this test, and by
/// `docs/traces/phase-labelled-generation-6b3edaa.txt`, whose first
/// `[blink] Progress:` line (88) comes long after `sampling using` (73).
///
/// So this asserts what is actually true of the mechanism: the callback is
/// installed with `expected_steps == 0`, the context is created and dropped
/// without panicking or leaking, and **any** event it does receive is labelled
/// `Loading` rather than mislabelled as sampling. The day eager loading is
/// adopted — or a stack loads eagerly — this becomes a real load-progress check
/// with no edit beyond tightening the count.
///
/// C2 clause (a), the half that carries the user-visible value, is C4 clause 1:
/// the lazy loads that used to render as fake generation progress are now
/// labelled `loading` inside the generation.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn model_load_callback_installs_and_tears_down() {
    use sd_wrapper::{CancelHandle, Phase, ProgressUpdate, SdContext};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    let started = std::time::Instant::now();
    /// step, total_steps, the phase Blink assigned it, seconds since the call.
    type LoadEvent = (u32, u32, Phase, f64);
    let seen: Arc<Mutex<Vec<LoadEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);

    let cb: sd_wrapper::progress::ProgressCallback = Box::new(move |u: ProgressUpdate| {
        sink.lock()
            .unwrap()
            .push((u.step, u.total_steps, u.phase, started.elapsed().as_secs_f64()));
    });

    let ctx = SdContext::with_cancel_flag(
        zimage_config(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(CancelHandle::new()),
        Some(cb),
    )
    .expect("failed to create sd.cpp context");
    let load_secs = started.elapsed().as_secs_f64();

    // Tear down while the callback is still owned by the inference thread.
    drop(ctx);

    let events = seen.lock().unwrap().clone();
    eprintln!(
        "[phase] context created in {load_secs:.2}s; load-window progress events: {}",
        events.len()
    );
    for (step, total, phase, at) in &events {
        eprintln!("[phase]   step={step}/{total} phase={phase:?} at {at:.2}s");
    }

    // Whatever arrives here must never be called sampling: `expected_steps == 0`
    // means there is no step count to compare against, so the machine defers to
    // the log markers, which `reset_phase()` has already put in `Loading`.
    for (step, total, phase, at) in &events {
        assert_eq!(
            *phase,
            Phase::Loading,
            "load-window event step={step}/{total} at {at:.2}s was labelled {phase:?}"
        );
    }
}

/// C3 — the warm path is not slowed by the phase machinery.
///
/// The bound is a no-regression bound against item 2's recorded warm auto_fit
/// median, not an absolute time: same test parameters, same machine, warm,
/// `--test-threads=1`, first run of the session discarded, median of at least
/// three.
///
/// **Finding, printed rather than asserted:** the first `sampling`-phase event
/// arrives roughly two thirds of the way into a warm run. That is not machinery
/// cost — it is the lazy weight read. Under item 2's `auto_fit` placement sd.cpp
/// chose `--params-backend disk`, so **every** generation re-streams the ~838
/// tensors (386 text-encoder + 452 diffusion) that this trace shows, and item 3
/// is what finally labels that window honestly instead of rendering it as
/// generation progress.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn warm_generation_does_not_regress_against_the_item_2_median() {
    use sd_wrapper::{Phase, ProgressUpdate};
    use std::sync::{Arc, Mutex};

    /// Item 2's recorded warm auto_fit median, seconds — measured on this
    /// machine with these parameters and committed with `e445b2c`
    /// ("feat: let sd.cpp auto_fit place model weights, replacing six perf
    /// toggles"). The gate there was the same 1.15x form.
    const ITEM_2_AUTO_FIT_MEDIAN_SECS: f64 = 4.1586;

    let ctx = zimage_cancel_context();

    const RUNS: usize = 4; // the first is discarded as cold
    let mut totals = Vec::new();
    let mut latencies = Vec::new();

    for i in 0..RUNS {
        let slot: Arc<Mutex<Option<f64>>> = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&slot);
        let t0 = std::time::Instant::now();
        let cb: sd_wrapper::progress::ProgressCallback = Box::new(move |u: ProgressUpdate| {
            if u.phase == Phase::Sampling {
                let mut s = sink.lock().unwrap();
                if s.is_none() {
                    *s = Some(t0.elapsed().as_secs_f64());
                }
            }
        });

        let started = std::time::Instant::now();
        ctx.txt2img(zimage_cancel_params(4, 42), Vec::new(), Some(cb), None)
            .unwrap_or_else(|e| panic!("run {i} failed: {e:?}"));
        let total = started.elapsed().as_secs_f64();
        let latency = slot
            .lock()
            .unwrap()
            .unwrap_or_else(|| panic!("run {i} never reported a sampling phase"));

        eprintln!(
            "[phase] run {i}: total={total:.4}s first_sampling_at={latency:.4}s ({:.0}% of the run)",
            latency / total * 100.0
        );
        if i > 0 {
            totals.push(total);
            latencies.push(latency);
        }
    }

    let mut sorted = totals.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let bound = 1.15 * ITEM_2_AUTO_FIT_MEDIAN_SECS;
    let latency_median = {
        let mut s = latencies.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s[s.len() / 2]
    };

    eprintln!("[phase] warm totals={totals:?} median={median:.4}s bound={bound:.4}s");
    eprintln!(
        "[phase] FINDING: first sampling event at a median of {latency_median:.4}s, \
         {:.0}% of the {median:.4}s run — the lazy re-read of ~838 tensors that \
         params-backend=disk pays on every generation.",
        latency_median / median * 100.0
    );

    assert!(
        median <= bound,
        "warm median {median:.4}s regressed past {bound:.4}s (1.15 x item 2's \
         recorded {ITEM_2_AUTO_FIT_MEDIAN_SECS:.4}s)"
    );
}
