import { Component, Show, onMount, onCleanup } from "solid-js";

interface InpaintToolbarProps {
  visible: boolean;
  brushSize: number;
  isEraser: boolean;
  onBrushSizeChange: (size: number) => void;
  onModeChange: (isEraser: boolean) => void;
  onClearMask: () => void;
}

const InpaintToolbar: Component<InpaintToolbarProps> = (props) => {
  const handleKeyDown = (e: KeyboardEvent) => {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;

    if (e.key === "b" || e.key === "B") {
      props.onModeChange(false);
    } else if (e.key === "e" || e.key === "E") {
      props.onModeChange(true);
    } else if (e.key === "[") {
      props.onBrushSizeChange(Math.max(10, props.brushSize - 5));
    } else if (e.key === "]") {
      props.onBrushSizeChange(Math.min(100, props.brushSize + 5));
    }
  };

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    window.removeEventListener("keydown", handleKeyDown);
  });

  const btnBase = {
    padding: "6px 14px",
    border: "1px solid var(--border)",
    "border-radius": "var(--radius)",
    "font-size": "13px",
    cursor: "pointer",
    transition: "var(--transition)",
    "white-space": "nowrap" as const,
  };

  const btnActive = {
    ...btnBase,
    background: "var(--accent)",
    color: "white",
    "border-color": "var(--accent)",
  };

  const btnInactive = {
    ...btnBase,
    background: "var(--bg-tertiary)",
    color: "var(--text-secondary)",
  };

  return (
    <Show when={props.visible}>
      <div
        style={{
          display: "flex",
          "align-items": "center",
          gap: "10px",
          padding: "8px 12px",
          background: "var(--bg-secondary)",
          border: "1px solid var(--border)",
          "border-radius": "var(--radius)",
          "max-width": "700px",
          width: "100%",
          "box-sizing": "border-box",
          opacity: props.visible ? "1" : "0",
          transition: "opacity 0.2s ease",
        }}
      >
        {/* Mode toggle */}
        <button
          onClick={() => props.onModeChange(false)}
          style={!props.isEraser ? btnActive : btnInactive}
          title="Brush mode (B)"
        >
          ✏ Brush
        </button>
        <button
          onClick={() => props.onModeChange(true)}
          style={props.isEraser ? btnActive : btnInactive}
          title="Eraser mode (E)"
        >
          ◻ Eraser
        </button>

        {/* Divider */}
        <div
          style={{
            width: "1px",
            height: "24px",
            background: "var(--border)",
            "flex-shrink": "0",
          }}
        />

        {/* Brush size */}
        <div
          style={{
            display: "flex",
            "align-items": "center",
            gap: "8px",
            flex: "1",
          }}
        >
          <span
            style={{
              "font-size": "12px",
              color: "var(--text-secondary)",
              "white-space": "nowrap",
              "flex-shrink": "0",
            }}
          >
            Size
          </span>
          <input
            type="range"
            min={10}
            max={100}
            value={props.brushSize}
            onInput={(e) =>
              props.onBrushSizeChange(parseInt(e.currentTarget.value, 10))
            }
            style={{
              flex: "1",
              "min-width": "80px",
              "max-width": "160px",
              cursor: "pointer",
              "accent-color": "var(--accent)",
            }}
          />
          <span
            style={{
              "font-size": "12px",
              color: "var(--text-primary)",
              "min-width": "28px",
              "text-align": "right",
              "font-variant-numeric": "tabular-nums",
            }}
          >
            {props.brushSize}
          </span>
        </div>

        {/* Divider */}
        <div
          style={{
            width: "1px",
            height: "24px",
            background: "var(--border)",
            "flex-shrink": "0",
          }}
        />

        {/* Clear mask */}
        <button
          onClick={props.onClearMask}
          style={{
            ...btnInactive,
            color: "var(--error, #e05252)",
            "border-color": "var(--error, #e05252)",
          }}
          title="Clear mask"
        >
          Clear Mask
        </button>
      </div>
    </Show>
  );
};

export default InpaintToolbar;
