import { Component, Show, onMount, onCleanup } from "solid-js";
import type { PerfSettings, LoraConfig } from "../lib/types";
import ToggleOption from "./ToggleOption";
import LoraPicker from "./LoraPicker";

interface SettingsDrawerProps {
  open: boolean;
  onClose: () => void;
  // Generation settings
  steps: number; onStepsChange: (v: number) => void;
  cfgScale: number; onCfgChange: (v: number) => void;
  seed: number; onSeedChange: (v: number) => void;
  width: number; onWidthChange: (v: number) => void;
  height: number; onHeightChange: (v: number) => void;
  sampler: string; onSamplerChange: (v: string) => void;
  // img2img
  strength: number; onStrengthChange: (v: number) => void;
  showStrength: boolean;
  // Image conditioning (Kontext edit mode)
  imgCfg: number; onImgCfgChange: (v: number) => void;
  showImgCfg: boolean;
  // ControlNet
  controlNetEnabled: boolean; onControlNetChange: (v: boolean) => void;
  controlStrength: number; onControlStrengthChange: (v: number) => void;
  // LoRA
  loras: LoraConfig[]; onLorasChange: (v: LoraConfig[]) => void;
  // Performance
  perfSettings: PerfSettings; onPerfChange: (v: PerfSettings) => void;
  // HF Token
  hfToken: string | null; onHfTokenChange: (v: string | null) => void;
  // Anthropic API Key
  anthropicKey: string | null; onAnthropicKeyChange: (v: string | null) => void;
}

const inputStyle = {
  padding: "4px 8px",
  background: "var(--bg-secondary)",
  border: "1px solid var(--border)",
  "border-radius": "4px",
  color: "var(--text-primary)",
  width: "80px",
  "font-size": "13px",
  outline: "none",
} as const;

const labelStyle = {
  "font-size": "12px",
  color: "var(--text-secondary)",
  "min-width": "60px",
} as const;

const sectionHeadStyle = {
  "font-size": "11px",
  "font-weight": "600",
  color: "var(--text-secondary)",
  "text-transform": "uppercase" as const,
  "letter-spacing": "0.06em",
  "margin-bottom": "10px",
} as const;

const sectionStyle = {
  "border-top": "1px solid var(--border)",
  padding: "14px 20px",
} as const;

const rowStyle = {
  display: "flex",
  "align-items": "center",
  gap: "8px",
  "margin-bottom": "8px",
} as const;

const SettingsDrawer: Component<SettingsDrawerProps> = (props) => {
  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") props.onClose();
    };
    window.addEventListener("keydown", handler);
    onCleanup(() => window.removeEventListener("keydown", handler));
  });

  return (
    <div style={{
      position: "fixed",
      inset: "0",
      "z-index": "50",
      "pointer-events": props.open ? "auto" : "none",
    }}>
      {/* Backdrop */}
      <div
        onClick={props.onClose}
        style={{
          position: "absolute",
          inset: "0",
          background: "rgba(0,0,0,0.3)",
          opacity: props.open ? "1" : "0",
          transition: "opacity 0.25s ease",
          "pointer-events": props.open ? "auto" : "none",
        }}
      />

      {/* Panel */}
      <div style={{
        position: "absolute",
        right: "0",
        top: "0",
        bottom: "0",
        width: "320px",
        background: "var(--bg-primary)",
        "border-left": "1px solid var(--border)",
        "overflow-y": "auto",
        transform: props.open ? "translateX(0)" : "translateX(100%)",
        transition: "transform 0.25s cubic-bezier(0.4, 0, 0.2, 1)",
        "pointer-events": "auto",
        display: "flex",
        "flex-direction": "column",
      }}>

        {/* Header */}
        <div style={{
          display: "flex",
          "align-items": "center",
          "justify-content": "space-between",
          padding: "14px 20px",
          "border-bottom": "1px solid var(--border)",
          "flex-shrink": "0",
        }}>
          <span style={{ "font-size": "14px", "font-weight": "600", color: "var(--text-primary)" }}>
            Settings
          </span>
          <button
            onClick={props.onClose}
            style={{
              background: "none",
              border: "none",
              color: "var(--text-secondary)",
              cursor: "pointer",
              "font-size": "18px",
              padding: "2px 6px",
              "border-radius": "4px",
              "line-height": "1",
            }}
            title="Close"
          >
            ✕
          </button>
        </div>

        {/* Generation */}
        <div style={sectionStyle}>
          <div style={sectionHeadStyle}>Generation</div>

          <div style={rowStyle}>
            <span style={labelStyle}>Steps</span>
            <input
              type="number"
              min="1"
              max="100"
              value={props.steps}
              onInput={(e) => props.onStepsChange(parseInt(e.currentTarget.value) || 1)}
              style={inputStyle}
            />
          </div>

          <div style={rowStyle}>
            <span style={labelStyle}>CFG</span>
            <input
              type="number"
              min="0"
              max="20"
              step="0.5"
              value={props.cfgScale}
              onInput={(e) => props.onCfgChange(parseFloat(e.currentTarget.value) || 1)}
              style={inputStyle}
            />
          </div>

          <div style={rowStyle}>
            <span style={labelStyle}>Seed</span>
            <input
              type="number"
              min="-1"
              value={props.seed}
              onInput={(e) => props.onSeedChange(parseInt(e.currentTarget.value) ?? -1)}
              style={{ ...inputStyle, width: "100px" }}
            />
            <button
              onClick={() => props.onSeedChange(-1)}
              style={{
                padding: "4px 8px",
                background: "var(--bg-tertiary)",
                border: "1px solid var(--border)",
                "border-radius": "4px",
                color: "var(--text-secondary)",
                cursor: "pointer",
                "font-size": "11px",
              }}
            >
              Rand
            </button>
          </div>

          <div style={rowStyle}>
            <span style={labelStyle}>Size</span>
            <select
              value={`${props.width}x${props.height}`}
              onChange={(e) => {
                const [w, h] = e.currentTarget.value.split("x").map(Number);
                props.onWidthChange(w);
                props.onHeightChange(h);
              }}
              style={{ ...inputStyle, width: "120px" }}
            >
              <option value="512x512">512×512</option>
              <option value="512x768">512×768</option>
              <option value="768x512">768×512</option>
              <option value="768x768">768×768</option>
              <option value="1024x1024">1024×1024</option>
              <option value="1024x768">1024×768</option>
              <option value="768x1024">768×1024</option>
            </select>
          </div>

          <div style={{ ...rowStyle, "margin-bottom": "0" }}>
            <span style={labelStyle}>Sampler</span>
            <select
              value={props.sampler}
              onChange={(e) => props.onSamplerChange(e.currentTarget.value)}
              style={{ ...inputStyle, width: "120px" }}
            >
              <option value="euler">Euler</option>
              <option value="euler_a">Euler A</option>
              <option value="heun">Heun</option>
              <option value="dpm2">DPM2</option>
              <option value="dpm++2m">DPM++ 2M</option>
              <option value="lcm">LCM</option>
            </select>
          </div>
        </div>

        {/* img2img Strength */}
        <Show when={props.showStrength}>
          <div style={sectionStyle}>
            <div style={sectionHeadStyle}>img2img</div>
            <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
              <span style={labelStyle}>Strength</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={props.strength}
                onInput={(e) => props.onStrengthChange(parseFloat(e.currentTarget.value))}
                style={{ flex: "1" }}
              />
              <span style={{ "font-size": "13px", "min-width": "35px" }}>
                {props.strength.toFixed(2)}
              </span>
            </div>
          </div>
        </Show>

        {/* Image Conditioning */}
        <Show when={props.showImgCfg}>
          <div style={sectionStyle}>
            <div style={sectionHeadStyle}>Image Conditioning</div>
            <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
              <span style={{ ...labelStyle, "min-width": "80px" }}>Img CFG</span>
              <input
                type="range"
                min="0.5"
                max="3.0"
                step="0.1"
                value={props.imgCfg}
                onInput={(e) => props.onImgCfgChange(parseFloat(e.currentTarget.value))}
                style={{ flex: "1" }}
              />
              <span style={{ "font-size": "13px", "min-width": "35px" }}>
                {props.imgCfg.toFixed(2)}
              </span>
            </div>
          </div>
        </Show>

        {/* LoRA */}
        <div style={sectionStyle}>
          <div style={sectionHeadStyle}>LoRA</div>
          <LoraPicker
            loras={props.loras}
            onLorasChange={props.onLorasChange}
            visible={true}
          />
        </div>

        {/* ControlNet */}
        <div style={sectionStyle}>
          <div style={{
            display: "flex",
            "align-items": "center",
            gap: "8px",
            "margin-bottom": "10px",
          }}>
            <div style={sectionHeadStyle}>ControlNet</div>
            <Show when={props.controlNetEnabled}>
              <span style={{
                "font-size": "10px",
                color: "var(--warning)",
                background: "rgba(245, 158, 11, 0.1)",
                border: "1px solid rgba(245, 158, 11, 0.2)",
                "border-radius": "4px",
                padding: "1px 6px",
                "margin-bottom": "10px",
              }}>
                Requires model reload
              </span>
            </Show>
          </div>
          <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
            <ToggleOption
              label="Preserve Structure (Canny)"
              tooltip="Use ControlNet to preserve structural edges from the input image. Requires model reload when toggled."
              checked={props.controlNetEnabled}
              onChange={props.onControlNetChange}
            />
            <Show when={props.controlNetEnabled}>
              <div style={{ display: "flex", "align-items": "center", gap: "8px", "padding-left": "22px" }}>
                <span style={{ "font-size": "12px", color: "var(--text-secondary)", "min-width": "90px" }}>
                  Control Strength
                </span>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.05"
                  value={props.controlStrength}
                  onInput={(e) => props.onControlStrengthChange(parseFloat(e.currentTarget.value))}
                  style={{ flex: "1" }}
                />
                <span style={{ "font-size": "12px", "min-width": "35px" }}>
                  {props.controlStrength.toFixed(2)}
                </span>
              </div>
            </Show>
          </div>
        </div>

        {/* Performance */}
        <div style={sectionStyle}>
          <div style={sectionHeadStyle}>Performance</div>
          <div style={{ "font-size": "11px", color: "var(--text-secondary)", "margin-bottom": "10px", opacity: "0.7" }}>
            Changes apply on next model load
          </div>
          <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
            <ToggleOption
              label="Flash Attention"
              tooltip="Faster inference with lower memory usage. Recommended for all GPUs."
              checked={props.perfSettings.flash_attn}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, flash_attn: v, diffusion_flash_attn: v })}
            />
            <ToggleOption
              label="Memory-Mapped Loading"
              tooltip="Faster model loading via mmap. Recommended."
              checked={props.perfSettings.enable_mmap}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, enable_mmap: v })}
            />
            <div style={{
              "border-top": "1px solid var(--border)",
              "padding-top": "8px",
              "margin-top": "4px",
            }}>
              <span style={{ "font-size": "11px", color: "var(--warning)", "font-weight": "600" }}>
                VRAM Saving — only enable if models won't fit in GPU memory
              </span>
            </div>
            <ToggleOption
              label="Offload to CPU"
              tooltip="Offload model params to CPU RAM. SIGNIFICANTLY slower. Only for models that exceed your VRAM."
              checked={props.perfSettings.offload_params_to_cpu}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, offload_params_to_cpu: v })}
            />
            <ToggleOption
              label="Free Params Early"
              tooltip="Free weights after each stage. Slower but uses less peak VRAM."
              checked={props.perfSettings.free_params_immediately}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, free_params_immediately: v })}
            />
            <ToggleOption
              label="CLIP on CPU"
              tooltip="Run text encoder on CPU. Frees GPU VRAM but slower text processing."
              checked={props.perfSettings.keep_clip_on_cpu}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, keep_clip_on_cpu: v })}
            />
            <ToggleOption
              label="VAE on CPU"
              tooltip="Run VAE on CPU. Frees GPU VRAM but slower image decode."
              checked={props.perfSettings.keep_vae_on_cpu}
              onChange={(v) => props.onPerfChange({ ...props.perfSettings, keep_vae_on_cpu: v })}
            />
          </div>
        </div>

        {/* Account */}
        <div style={{ ...sectionStyle, "border-bottom": "none" }}>
          <div style={sectionHeadStyle}>Account</div>
          <div style={{ "font-size": "12px", color: "var(--text-secondary)", "font-weight": "600", "margin-bottom": "4px" }}>
            HuggingFace Token
          </div>
          <div style={{ "font-size": "11px", color: "var(--text-secondary)", "margin-bottom": "8px", opacity: "0.6" }}>
            Required for gated models (Flux VAE)
          </div>
          <input
            type="password"
            placeholder="hf_..."
            value={props.hfToken ?? ""}
            onInput={(e) => props.onHfTokenChange(e.currentTarget.value || null)}
            style={{
              width: "100%",
              padding: "6px 10px",
              background: "var(--bg-secondary)",
              border: "1px solid var(--border)",
              "border-radius": "4px",
              color: "var(--text-primary)",
              "font-size": "13px",
              outline: "none",
              "box-sizing": "border-box",
            }}
          />
          <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "6px" }}>
            huggingface.co/settings/tokens
          </div>

          <div style={{ "font-size": "12px", color: "var(--text-secondary)", "font-weight": "600", "margin-bottom": "4px", "margin-top": "16px" }}>
            Anthropic API Key
          </div>
          <div style={{ "font-size": "11px", color: "var(--text-secondary)", "margin-bottom": "8px", opacity: "0.6" }}>
            Required for prompt enhancement (✨)
          </div>
          <input
            type="password"
            placeholder="sk-ant-..."
            value={props.anthropicKey ?? ""}
            onInput={(e) => props.onAnthropicKeyChange(e.currentTarget.value || null)}
            style={{
              width: "100%",
              padding: "6px 10px",
              background: "var(--bg-secondary)",
              border: "1px solid var(--border)",
              "border-radius": "4px",
              color: "var(--text-primary)",
              "font-size": "13px",
              outline: "none",
              "box-sizing": "border-box",
            }}
          />
          <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "6px" }}>
            console.anthropic.com/settings/keys
          </div>
        </div>

      </div>
    </div>
  );
};

export default SettingsDrawer;
