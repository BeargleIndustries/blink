//! Generation progress, and the machine that tells model loading apart from
//! sampling and decoding.
//!
//! # Why this exists
//!
//! sd.cpp funnels both phases through the *same* global progress callback.
//! `pretty_progress` (`src/core/util.cpp:536`) and `pretty_bytes_progress`
//! (`:553`) both end in `sd_progress_cb(step, steps, time, data)`, and
//! `pretty_bytes_progress` is what `model_loader.cpp:1208`/`:1226` call with
//! `step = tensors loaded, steps = total tensors`. From the callback alone a
//! 386-of-386 tensor load is indistinguishable from a 386-step sample.
//!
//! # The rule
//!
//! The **step count is authoritative** and runs on every progress event; log
//! markers are one-way upgrades on top of it. Two facts from the vendored source
//! force that arrangement, and both are visible in
//! `docs/traces/cold-first-generation-6b3edaa.txt`:
//!
//! 1. Blink's flagship stacks never log `loading model from '` — they pass
//!    `diffusion_model_path`, so the line is `loading diffusion model from '`
//!    (`stable-diffusion.cpp:721`, trace line 25).
//! 2. `eager_load` defaults to `false` (`stable-diffusion.cpp:252`), so weights
//!    stream in lazily *inside* `generate_image`, which is downstream of
//!    `sampling using %s method` (`:4397`, trace line 77). A design in which log
//!    markers are authoritative reports `Sampling` for the whole cold load.
//!
//! Hence: `sampling using ` may upgrade the phase but must **never** latch it.
//! Only a real sampling *progress* event latches.
//!
//! # The step-index clause
//!
//! `sample()` emits its own opening `pretty_progress(0, steps, 0)` tick
//! (`stable-diffusion.cpp:2643`, trace line 103) **before** the lazy diffusion
//! load (trace lines 104-112). It carries `total_steps == expected_steps` by
//! construction, for every model and every step count. Treating it as sampling
//! would latch immediately and mislabel the load that dominates a cold start —
//! which is the entire point of this module. So a `step == 0` tick is never real
//! sampling; only `step >= 1` is.
//!
//! # Scope
//!
//! The step-count rule is bounded to **single-stage** models. sd.cpp computes
//! `total_steps = sample_steps + std::max(0, high_noise_sample_steps)`
//! (`stable-diffusion.cpp:4352`) while `expected_steps` here is Blink's requested
//! step count alone, so on a high-noise/MoE two-stage model the two can never be
//! equal and the phase would stay `Loading` for the whole generation. Fix
//! `expected_steps` before cataloguing such a model; do not patch the latch.
//!
//! # Why the state is process-global
//!
//! sd.cpp's log and progress callbacks are themselves global
//! (`src/core/util.cpp:342`), and Blink runs exactly one context at a time
//! (`AppState::sd_context: Mutex<Option<SdContext>>`). **If that invariant ever
//! changes, this state must move into the trampoline data.**

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// What the engine is actually doing when a progress event arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Reading weights off disk. Progress counts tensors, not steps.
    Loading,
    /// Real sampling. Progress counts the steps the user asked for.
    Sampling,
    /// VAE decode. Progress counts latents or tiles.
    Decoding,
}

impl Phase {
    /// The wire name used by the Tauri event and the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Loading => "loading",
            Phase::Sampling => "sampling",
            Phase::Decoding => "decoding",
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Phase::Loading => 0,
            Phase::Sampling => 1,
            Phase::Decoding => 2,
        }
    }

    fn from_u8(v: u8) -> Phase {
        match v {
            1 => Phase::Sampling,
            2 => Phase::Decoding,
            _ => Phase::Loading,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub step: u32,
    pub total_steps: u32,
    pub elapsed_secs: f32,
    pub preview: Option<Vec<u8>>,
    pub phase: Phase,
}

pub type ProgressCallback = Box<dyn Fn(ProgressUpdate) + Send>;

static PHASE: AtomicU8 = AtomicU8::new(0); // Phase::Loading
static LATCHED: AtomicBool = AtomicBool::new(false);

/// The phase as last recorded. Used directly only in the `new_sd_ctx` window,
/// where no step count exists to reason from.
pub fn current_phase() -> Phase {
    Phase::from_u8(PHASE.load(Ordering::SeqCst))
}

fn set_phase(phase: Phase) {
    PHASE.store(phase.to_u8(), Ordering::SeqCst);
}

/// Back to `Loading`, latch cleared. Called at the start of every generation and
/// every context creation.
pub fn reset_phase() {
    LATCHED.store(false, Ordering::SeqCst);
    set_phase(Phase::Loading);
}

/// Classify one sd.cpp log line.
///
/// **Pure and upgrade-only by contract** — the caller ([`note_log_line`])
/// enforces the one-way rule.
///
/// The two `Loading` arms are **canaries, not signals**: [`note_log_line`]
/// accepts `Loading` only when the phase is already `Loading`, and
/// [`reset_phase`] guarantees exactly that at the start of every generation and
/// context creation, so their result is always discarded at runtime. They are
/// matched and unit-tested purely so that an upstream rewording fails a fast CPU
/// test — the early warning that the committed transcript has gone stale. The
/// mechanism that actually labels the loading phase is [`note_progress`].
pub fn phase_from_log_line(line: &str) -> Option<Phase> {
    // Upgrades first: these are the two markers with a runtime role, and
    // `decoding … latents` must win even though no load marker can collide.
    if line.contains("sampling using ") {
        return Some(Phase::Sampling);
    }
    if line.contains("decoding ") && line.contains(" latents") {
        return Some(Phase::Decoding);
    }
    // Canaries. `model from '` covers `loading model from '` (:714),
    // `loading diffusion model from '` (:721), `loading high noise diffusion
    // model from '` (:728) and `loading unconditional diffusion model from '`
    // (:735). The encoder stacks are logged under their own names.
    if line.contains("model from '")
        || line.contains("loading clip_l from '")
        || line.contains("loading t5xxl from '")
        || line.contains("loading llm from '")
    {
        return Some(Phase::Loading);
    }
    None
}

/// Apply a log line as a **one-way upgrade** only.
///
/// `Loading` is accepted only when the phase is already `Loading`, which is what
/// makes the two `Loading` arms of [`phase_from_log_line`] inert.
///
/// `Sampling` updates the stored phase but **must never set the latch**:
/// `sampling using %s method` prints at `stable-diffusion.cpp:4397`, which under
/// `eager_load = false` (`:252`) is *before* the lazy load. Latching there would
/// suppress `Loading` for the whole cold read — the defect this module exists to
/// prevent. `Decoding` does latch, because `decoding %zu latents` is logged from
/// `decode_image_outputs` (`:5340`), which by construction runs after sampling
/// has finished, so there is no load left to mislabel.
pub fn note_log_line(line: &str) {
    match phase_from_log_line(line) {
        Some(Phase::Loading) => {
            // Never a downgrade.
            if current_phase() == Phase::Loading {
                set_phase(Phase::Loading);
            }
        }
        Some(Phase::Sampling) => set_phase(Phase::Sampling),
        Some(Phase::Decoding) => {
            set_phase(Phase::Decoding);
            LATCHED.store(true, Ordering::SeqCst);
        }
        None => {}
    }
}

/// The authoritative rule, run on **every** progress event.
///
/// `expected_steps` is the step count Blink asked for, taken from
/// `GenerationParams::steps` before sd.cpp sees it. `0` means "no step count
/// applies here" — the callback installed around `new_sd_ctx`.
pub fn note_progress(step: u32, total_steps: u32, expected_steps: u32) -> Phase {
    if expected_steps != 0 && (total_steps != expected_steps || step == 0) {
        // A tensor count, or `sample()`'s opening `step == 0` tick. Either way
        // not real sampling — unless a real sampling event already latched, in
        // which case a lazy mid-generation load must not flicker the label.
        if LATCHED.load(Ordering::SeqCst) {
            return current_phase();
        }
        set_phase(Phase::Loading);
        return Phase::Loading;
    }

    if expected_steps != 0 {
        // total_steps == expected_steps && step >= 1: real sampling.
        set_phase(Phase::Sampling);
        LATCHED.store(true, Ordering::SeqCst);
        return Phase::Sampling;
    }

    // The `new_sd_ctx` window. No step count to reason from, so defer to
    // whatever the log markers have recorded — which `reset_phase()` has already
    // set to `Loading`.
    //
    // This branch deliberately does not consult `LATCHED`, which is only sound
    // because it is reachable only from the callback installed around
    // `new_sd_ctx` — that runs once, on the inference thread, before any command
    // is dequeued, and both latch-setting paths (`SdCppContext::generate` for
    // txt2img and img2img) call `reset_phase()` first. Video bypasses this module
    // entirely (`video::generate_video` installs its own trampoline, which never
    // calls `note_progress`). If `SdCppContext::new` ever gains another caller,
    // or video is wired in here, make this branch respect `LATCHED`.
    current_phase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The phase machine is process-global by design, so the tests that drive it
    /// must not run concurrently with each other.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn guard() -> std::sync::MutexGuard<'static, ()> {
        let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_phase();
        g
    }

    // -----------------------------------------------------------------------
    // Canary tests.
    //
    // These three assert `phase_from_log_line`'s return value, which the runtime
    // DISCARDS (see the doc comment on that function). Their job is to fail when
    // upstream rewords a load line — the early warning that
    // `docs/traces/cold-first-generation-6b3edaa.txt` has gone stale. They are
    // NOT evidence that the loading phase is detected; `note_progress` does that,
    // and acceptance criterion C4 proves it over a real transcript. Do not
    // "fix" the design to match what these tests imply.
    //
    // Every literal below is copied from that transcript, not from the C++.
    // -----------------------------------------------------------------------

    #[test]
    fn canary_phase_from_log_line_detects_diffusion_model_load() {
        // Trace line 25 — the marker revision 1 of the plan got wrong. Blink's
        // flagship stacks log THIS, never `loading model from '`.
        let line = "stable-diffusion.cpp:721  - loading diffusion model from \
                    'C:\\Users\\Brad\\AppData\\Roaming\\com.beargle.blink\\models\\z-image\\z_image_turbo-Q8_0.gguf'";
        assert_eq!(phase_from_log_line(line), Some(Phase::Loading));
    }

    #[test]
    fn canary_phase_from_log_line_detects_single_file_model_load() {
        // The SD1.5/SDXL shape (`stable-diffusion.cpp:714`). It does not appear
        // in the reference transcript because the Z-Image stack never emits it,
        // so the path is taken from trace line 25 and the prefix from the source
        // line the `model from '` substring is designed to cover.
        let line = "stable-diffusion.cpp:714  - loading model from \
                    'C:\\Users\\Brad\\AppData\\Roaming\\com.beargle.blink\\models\\z-image\\z_image_turbo-Q8_0.gguf'";
        assert_eq!(phase_from_log_line(line), Some(Phase::Loading));
    }

    #[test]
    fn canary_phase_from_log_line_detects_text_encoder_load() {
        // Trace line 28. The reference stack loads an LLM text encoder, not
        // clip_l — which is exactly the class of mistake the canaries exist to
        // catch. clip_l/t5xxl are matched too, for the Flux stacks.
        let line = "stable-diffusion.cpp:778  - loading llm from \
                    'C:\\Users\\Brad\\AppData\\Roaming\\com.beargle.blink\\models\\z-image\\Qwen3-4B-Instruct-2507-Q4_K_M.gguf'";
        assert_eq!(phase_from_log_line(line), Some(Phase::Loading));
        assert_eq!(
            phase_from_log_line("loading clip_l from 'clip_l.safetensors'"),
            Some(Phase::Loading)
        );
        assert_eq!(
            phase_from_log_line("loading t5xxl from 't5xxl.safetensors'"),
            Some(Phase::Loading)
        );
    }

    #[test]
    fn phase_from_log_line_detects_sampling() {
        // Trace line 77.
        assert_eq!(
            phase_from_log_line("stable-diffusion.cpp:4397 - sampling using Euler method"),
            Some(Phase::Sampling)
        );
    }

    #[test]
    fn phase_from_log_line_detects_decoding() {
        // Trace line 123.
        assert_eq!(
            phase_from_log_line("stable-diffusion.cpp:5342 - decoding 1 latents"),
            Some(Phase::Decoding)
        );
    }

    #[test]
    fn phase_from_log_line_ignores_unrelated_lines() {
        // Trace lines 67 and 96, plus a DEBUG tensor line (line 90) — none of
        // which may move the phase.
        for line in [
            "stable-diffusion.cpp:1761 - total params memory size = 9988.08MB (VRAM 9988.08MB, RAM 0.00MB)",
            "model_loader.cpp:1243 - loading tensors completed, taking 0.98s",
            "model_loader.cpp:983 - loading 386/398 tensors from Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            "stable-diffusion.cpp:5700 - generating image: 1/1 - seed 42",
        ] {
            assert_eq!(phase_from_log_line(line), None, "unexpected match: {line}");
        }
    }

    // -----------------------------------------------------------------------
    // The authoritative rule.
    // -----------------------------------------------------------------------

    #[test]
    fn note_progress_reports_loading_when_total_steps_differs() {
        let _g = guard();
        // Trace line 93: a tensor count, not a step count.
        assert_eq!(note_progress(100, 386, 4), Phase::Loading);
    }

    #[test]
    fn note_progress_reports_sampling_on_matching_step_count() {
        let _g = guard();
        // Trace line 116.
        assert_eq!(note_progress(1, 4, 4), Phase::Sampling);
    }

    #[test]
    fn note_progress_latches_sampling_against_later_loads() {
        let _g = guard();
        assert_eq!(note_progress(1, 4, 4), Phase::Sampling);
        // A lazy ControlNet/VAE load inside the same generation must not flicker
        // the label back to Loading.
        assert_eq!(note_progress(40, 452, 4), Phase::Sampling);
    }

    #[test]
    fn sample_step_zero_tick_does_not_latch() {
        let _g = guard();
        // Replays trace lines 103 -> 105 -> 116. This is the regression test for
        // the Step 0 finding, and it fails against the two-argument rule that
        // did not look at `step`.
        assert_eq!(note_progress(0, 4, 4), Phase::Loading, "trace line 103");
        assert!(
            !LATCHED.load(Ordering::SeqCst),
            "sample()'s opening step=0 tick must not latch"
        );
        assert_eq!(note_progress(40, 452, 4), Phase::Loading, "trace line 106");
        assert_eq!(note_progress(1, 4, 4), Phase::Sampling, "trace line 116");
        assert!(LATCHED.load(Ordering::SeqCst), "real sampling must latch");
    }

    #[test]
    fn sampling_marker_alone_does_not_latch() {
        let _g = guard();
        // The only test that enters through the MARKER path. It is what fails if
        // anyone re-adds LATCHED to note_log_line's Sampling arm.
        note_log_line("stable-diffusion.cpp:4397 - sampling using Euler method");
        assert_eq!(current_phase(), Phase::Sampling, "the marker upgrades the phase");
        assert_eq!(
            note_progress(100, 386, 4),
            Phase::Loading,
            "the lazy load after the sampling marker must still read as loading"
        );
    }

    #[test]
    fn decoding_marker_does_latch() {
        let _g = guard();
        // The deliberate asymmetry with the test above: `decoding … latents` is
        // logged after sampling completes, so there is no load left to mislabel.
        note_log_line("stable-diffusion.cpp:5342 - decoding 1 latents");
        assert_eq!(note_progress(386, 386, 4), Phase::Decoding);
    }

    #[test]
    fn log_markers_cannot_downgrade_sampling_to_loading() {
        let _g = guard();
        assert_eq!(note_progress(4, 4, 4), Phase::Sampling);
        note_log_line("stable-diffusion.cpp:778  - loading llm from 'Qwen3-4B.gguf'");
        assert_eq!(current_phase(), Phase::Sampling);
    }

    #[test]
    fn reset_phase_clears_the_latch() {
        let _g = guard();
        assert_eq!(note_progress(1, 4, 4), Phase::Sampling);
        assert!(LATCHED.load(Ordering::SeqCst));
        reset_phase();
        assert!(!LATCHED.load(Ordering::SeqCst));
        assert_eq!(current_phase(), Phase::Loading);
        assert_eq!(note_progress(100, 386, 4), Phase::Loading);
    }

    #[test]
    fn context_creation_window_defers_to_the_log_markers() {
        let _g = guard();
        // expected_steps == 0: the new_sd_ctx window, where the tensor counts are
        // all sd.cpp reports and there is no step count to compare against.
        assert_eq!(note_progress(386, 386, 0), Phase::Loading);
        note_log_line("stable-diffusion.cpp:721  - loading diffusion model from 'z_image_turbo-Q8_0.gguf'");
        assert_eq!(note_progress(0, 452, 0), Phase::Loading);
    }
}
