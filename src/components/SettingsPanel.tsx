import { Component, Show, createSignal } from "solid-js";

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
        </div>
      </Show>
    </div>
  );
};

export default SettingsPanel;
