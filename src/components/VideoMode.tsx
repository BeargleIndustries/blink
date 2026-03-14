import { Component, createSignal, Show } from "solid-js";
import type { VideoRequest } from "../lib/types";

interface VideoModeProps {
  onGenerate: (request: VideoRequest) => void;
  onCancel: () => void;
  generating: boolean;
  modelReady: boolean;
}

const VideoMode: Component<VideoModeProps> = (props) => {
  const [prompt, setPrompt] = createSignal("");
  const [negativePrompt, setNegativePrompt] = createSignal("");
  const [showNeg, setShowNeg] = createSignal(false);
  const [width, setWidth] = createSignal(512);
  const [height, setHeight] = createSignal(512);
  const [frames, setFrames] = createSignal(16);
  const [strength, setStrength] = createSignal(0.75);
  const [seed, setSeed] = createSignal(-1);
  const [inputImage, setInputImage] = createSignal<string | null>(null);
  const [dragOver, setDragOver] = createSignal(false);

  const canGenerate = () =>
    props.modelReady && !props.generating && prompt().trim().length > 0;

  const handleGenerate = () => {
    if (!canGenerate()) return;

    let inputBytes: number[] | undefined;
    if (inputImage()) {
      const raw = inputImage()!;
      const base64 = raw.includes(",") ? raw.split(",")[1] : raw;
      const binary = atob(base64);
      inputBytes = Array.from(binary, (c) => c.charCodeAt(0));
    }

    const request: VideoRequest = {
      prompt: prompt(),
      negative_prompt: negativePrompt() || undefined,
      width: width(),
      height: height(),
      seed: seed(),
      video_frames: frames(),
      strength: inputBytes ? strength() : undefined,
      input_image: inputBytes,
    };

    props.onGenerate(request);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleGenerate();
    }
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer?.files?.[0];
    if (!file || !file.type.startsWith("image/")) return;
    const reader = new FileReader();
    reader.onload = (ev) => setInputImage(ev.target?.result as string);
    reader.readAsDataURL(file);
  };

  const inputStyle = {
    width: "100%",
    padding: "8px 12px",
    background: "var(--bg-secondary)",
    border: "1px solid var(--border)",
    "border-radius": "var(--radius)",
    color: "var(--text-primary)",
    "font-size": "13px",
    outline: "none",
    "box-sizing": "border-box" as const,
    "font-family": "inherit",
  };

  const labelStyle = {
    "font-size": "11px",
    "font-weight": "600",
    color: "var(--text-secondary)",
    "text-transform": "uppercase" as const,
    "letter-spacing": "0.06em",
  };

  return (
    <div style={{
      width: "100%",
      "max-width": "700px",
      display: "flex",
      "flex-direction": "column",
      gap: "10px",
    }}>
      {/* Prompt */}
      <textarea
        placeholder="Describe the video you want to generate…"
        rows={3}
        value={prompt()}
        onInput={(e) => setPrompt(e.currentTarget.value)}
        onKeyDown={handleKeyDown}
        disabled={props.generating}
        style={{
          ...inputStyle,
          resize: "vertical",
          "font-size": "14px",
        }}
      />

      {/* Negative prompt collapsible */}
      <div>
        <button
          onClick={() => setShowNeg((v) => !v)}
          style={{
            display: "flex",
            "align-items": "center",
            gap: "5px",
            background: "none",
            border: "none",
            color: "var(--text-secondary)",
            "font-size": "12px",
            cursor: "pointer",
            padding: "0",
          }}
        >
          <svg
            width="10" height="10" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2.5" stroke-linecap="round"
            style={{ transform: showNeg() ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 0.15s" }}
          >
            <polyline points="9 18 15 12 9 6" />
          </svg>
          Negative prompt
        </button>
        <Show when={showNeg()}>
          <input
            type="text"
            placeholder="What to avoid…"
            value={negativePrompt()}
            onInput={(e) => setNegativePrompt(e.currentTarget.value)}
            disabled={props.generating}
            style={{ ...inputStyle, "margin-top": "6px" }}
          />
        </Show>
      </div>

      {/* Settings row */}
      <div style={{
        display: "grid",
        "grid-template-columns": "1fr 1fr 1fr 1fr",
        gap: "8px",
      }}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <span style={labelStyle}>Width</span>
          <input
            type="number"
            value={width()}
            onInput={(e) => setWidth(Number(e.currentTarget.value))}
            min={64} max={2048} step={64}
            disabled={props.generating}
            style={inputStyle}
          />
        </div>
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <span style={labelStyle}>Height</span>
          <input
            type="number"
            value={height()}
            onInput={(e) => setHeight(Number(e.currentTarget.value))}
            min={64} max={2048} step={64}
            disabled={props.generating}
            style={inputStyle}
          />
        </div>
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <span style={labelStyle}>Frames</span>
          <input
            type="number"
            value={frames()}
            onInput={(e) => setFrames(Number(e.currentTarget.value))}
            min={1} max={120}
            disabled={props.generating}
            style={inputStyle}
          />
        </div>
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <span style={labelStyle}>Seed</span>
          <input
            type="number"
            value={seed()}
            onInput={(e) => setSeed(Number(e.currentTarget.value))}
            disabled={props.generating}
            style={inputStyle}
          />
        </div>
      </div>

      {/* Img2vid section */}
      <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
        <span style={labelStyle}>Reference image (optional — img2vid)</span>
        <div
          onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
          style={{
            position: "relative",
            height: inputImage() ? "auto" : "72px",
            border: `1px dashed ${dragOver() ? "var(--accent)" : "var(--border)"}`,
            "border-radius": "var(--radius)",
            background: dragOver() ? "rgba(var(--accent-rgb, 99,102,241),0.06)" : "var(--bg-secondary)",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            transition: "var(--transition)",
            overflow: "hidden",
            cursor: "pointer",
          }}
          onClick={() => {
            if (inputImage()) return;
            const el = document.createElement("input");
            el.type = "file";
            el.accept = "image/*";
            el.onchange = () => {
              const file = el.files?.[0];
              if (!file) return;
              const reader = new FileReader();
              reader.onload = (ev) => setInputImage(ev.target?.result as string);
              reader.readAsDataURL(file);
            };
            el.click();
          }}
        >
          <Show when={inputImage()} fallback={
            <span style={{ color: "var(--text-secondary)", "font-size": "12px" }}>
              Drop image here or click to browse
            </span>
          }>
            <div style={{ position: "relative", width: "100%" }}>
              <img
                src={inputImage()!}
                style={{ display: "block", width: "100%", "max-height": "160px", "object-fit": "contain" }}
                alt="Reference"
              />
              <button
                onClick={(e) => { e.stopPropagation(); setInputImage(null); }}
                style={{
                  position: "absolute",
                  top: "6px",
                  right: "6px",
                  width: "22px",
                  height: "22px",
                  "border-radius": "50%",
                  background: "rgba(0,0,0,0.6)",
                  border: "none",
                  color: "white",
                  "font-size": "13px",
                  cursor: "pointer",
                  display: "flex",
                  "align-items": "center",
                  "justify-content": "center",
                  "line-height": "1",
                }}
              >
                ×
              </button>
            </div>
          </Show>
        </div>

        {/* Strength — only shown when reference image is set */}
        <Show when={!!inputImage()}>
          <div style={{ display: "flex", "align-items": "center", gap: "10px" }}>
            <span style={labelStyle}>Strength</span>
            <input
              type="range"
              min={0} max={1} step={0.05}
              value={strength()}
              onInput={(e) => setStrength(Number(e.currentTarget.value))}
              disabled={props.generating}
              style={{ flex: "1" }}
            />
            <span style={{ "font-size": "12px", color: "var(--text-secondary)", "min-width": "32px", "text-align": "right" }}>
              {strength().toFixed(2)}
            </span>
          </div>
        </Show>
      </div>

      {/* Generate / Cancel */}
      <button
        onClick={props.generating ? props.onCancel : handleGenerate}
        disabled={!props.generating && !canGenerate()}
        style={{
          padding: "12px 24px",
          background: props.generating ? "var(--error)" : "var(--accent)",
          border: "none",
          "border-radius": "var(--radius)",
          color: "white",
          "font-size": "15px",
          "font-weight": "bold",
          cursor: !props.generating && !canGenerate() ? "not-allowed" : "pointer",
          opacity: !props.generating && !canGenerate() ? "0.5" : "1",
          transition: "var(--transition)",
        }}
      >
        {!props.modelReady
          ? "Select a Wan model"
          : props.generating
          ? "Cancel"
          : "Generate Video"}
      </button>
    </div>
  );
};

export default VideoMode;
