import { Component, Show, createSignal } from "solid-js";
import type { PerfSettings } from "../lib/types";

interface SettingsPanelProps {
  steps: number;
  cfgScale: number;
  seed: number;
  width: number;
  height: number;
  sampler: string;
  onStepsChange: (v: number) => void;
  onCfgChange: (v: number) => void;
  onSeedChange: (v: number) => void;
  onWidthChange: (v: number) => void;
  onHeightChange: (v: number) => void;
  onSamplerChange: (v: string) => void;
  strength?: number;
  onStrengthChange?: (v: number) => void;
  showStrength: boolean;
  perfSettings: PerfSettings;
  onPerfChange: (settings: PerfSettings) => void;
  hfToken: string | null;
  onHfTokenChange: (token: string | null) => void;
}

const SettingsPanel: Component<SettingsPanelProps> = (props) => {
  const [expanded, setExpanded] = createSignal(false);

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

  return (
    <div style={{ width: "100%", "max-width": "700px" }}>
      <Show when={props.showStrength}>
        <div style={{
          display: "flex",
          "align-items": "center",
          gap: "8px",
          "margin-bottom": "8px",
          padding: "8px 12px",
          background: "var(--bg-secondary)",
          "border-radius": "var(--radius)",
        }}>
          <span style={labelStyle}>Strength</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={props.strength ?? 0.75}
            onInput={(e) =>
              props.onStrengthChange?.(parseFloat(e.currentTarget.value))
            }
            style={{ flex: "1" }}
          />
          <span style={{ "font-size": "13px", "min-width": "35px" }}>
            {(props.strength ?? 0.75).toFixed(2)}
          </span>
        </div>
      </Show>

      <button
        onClick={() => setExpanded(!expanded())}
        style={{
          background: "none",
          border: "none",
          color: "var(--text-secondary)",
          cursor: "pointer",
          "font-size": "13px",
          padding: "4px 0",
          display: "flex",
          "align-items": "center",
          gap: "4px",
        }}
      >
        <span style={{
          transform: expanded() ? "rotate(90deg)" : "rotate(0deg)",
          transition: "transform 0.2s",
          display: "inline-block",
        }}>
          &#9658;
        </span>
        Advanced Settings
      </button>

      <Show when={expanded()}>
        <div style={{
          display: "grid",
          "grid-template-columns": "1fr 1fr",
          gap: "8px",
          padding: "12px",
          background: "var(--bg-secondary)",
          "border-radius": "var(--radius)",
          "margin-top": "4px",
        }}>
          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
            <span style={labelStyle}>Steps</span>
            <input
              type="number"
              min="1"
              max="150"
              value={props.steps}
              onInput={(e) =>
                props.onStepsChange(parseInt(e.currentTarget.value) || 1)
              }
              style={inputStyle}
            />
          </div>

          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
            <span style={labelStyle}>CFG</span>
            <input
              type="number"
              min="1"
              max="30"
              step="0.5"
              value={props.cfgScale}
              onInput={(e) =>
                props.onCfgChange(parseFloat(e.currentTarget.value) || 1)
              }
              style={inputStyle}
            />
          </div>

          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
            <span style={labelStyle}>Seed</span>
            <input
              type="number"
              min="-1"
              value={props.seed}
              onInput={(e) =>
                props.onSeedChange(parseInt(e.currentTarget.value) ?? -1)
              }
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

          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
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
              <option value="512x512">512x512</option>
              <option value="512x768">512x768</option>
              <option value="768x512">768x512</option>
              <option value="768x768">768x768</option>
              <option value="1024x1024">1024x1024</option>
              <option value="1024x768">1024x768</option>
              <option value="768x1024">768x1024</option>
            </select>
          </div>

          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
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

          {/* HuggingFace Token section */}
          <div style={{
            "grid-column": "1 / -1",
            "border-top": "1px solid var(--border)",
            "padding-top": "8px",
            "margin-top": "4px",
          }}>
            <span style={{ "font-size": "12px", color: "var(--text-secondary)", "font-weight": "600" }}>
              HuggingFace Token
            </span>
            <span style={{ "font-size": "11px", color: "var(--text-secondary)", "margin-left": "8px", opacity: "0.6" }}>
              Required for gated models (Flux VAE)
            </span>
            <div style={{ display: "flex", gap: "8px", "margin-top": "8px", "align-items": "center" }}>
              <input
                type="password"
                placeholder="hf_..."
                value={props.hfToken ?? ""}
                onInput={(e) => props.onHfTokenChange(e.currentTarget.value || null)}
                style={{
                  flex: "1",
                  padding: "4px 8px",
                  background: "var(--bg-primary)",
                  border: "1px solid var(--border)",
                  "border-radius": "4px",
                  color: "var(--text-primary)",
                  "font-size": "13px",
                  outline: "none",
                }}
              />
            </div>
            <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "4px" }}>
              Get your token at huggingface.co/settings/tokens
            </div>
          </div>

          {/* Performance section — spans full width */}
          <div style={{
            "grid-column": "1 / -1",
            "border-top": "1px solid var(--border)",
            "padding-top": "8px",
            "margin-top": "4px",
          }}>
            <span style={{ "font-size": "12px", color: "var(--text-secondary)", "font-weight": "600" }}>
              Performance
            </span>
            <span style={{ "font-size": "11px", color: "var(--text-secondary)", "margin-left": "8px", opacity: "0.6" }}>
              Changes apply on next model load
            </span>
            <div style={{
              display: "grid",
              "grid-template-columns": "1fr 1fr",
              gap: "6px",
              "margin-top": "8px",
            }}>
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
                "grid-column": "1 / -1",
                "margin-top": "8px",
                "padding-top": "6px",
                "border-top": "1px solid var(--border)",
              }}>
                <span style={{ "font-size": "11px", color: "var(--warning)", "font-weight": "600" }}>
                  VRAM Saving (slows generation — only enable if models won't fit in GPU memory)
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
        </div>
      </Show>
    </div>
  );
};

const ToggleOption: Component<{
  label: string;
  tooltip: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}> = (props) => (
  <label
    title={props.tooltip}
    style={{
      display: "flex",
      "align-items": "center",
      gap: "6px",
      cursor: "pointer",
      "font-size": "12px",
      color: "var(--text-primary)",
    }}
  >
    <input
      type="checkbox"
      checked={props.checked}
      onChange={(e) => props.onChange(e.currentTarget.checked)}
      style={{ "accent-color": "var(--accent)" }}
    />
    {props.label}
  </label>
);

export default SettingsPanel;
