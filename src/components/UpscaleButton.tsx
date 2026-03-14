import { Component, createSignal, Show } from "solid-js";
import { upscaleImage } from "../lib/tauri-api";

interface UpscaleButtonProps {
  imageData: string | null;
  onUpscaled: (upscaledBase64: string) => void;
  visible: boolean;
}

const UpscaleButton: Component<UpscaleButtonProps> = (props) => {
  const [upscaling, setUpscaling] = createSignal(false);
  const [showMenu, setShowMenu] = createSignal(false);
  const [errorMsg, setErrorMsg] = createSignal<string | null>(null);

  const disabled = () => !props.imageData || upscaling();

  const handleUpscale = async (factor: 2 | 4) => {
    if (disabled()) return;
    setShowMenu(false);
    setUpscaling(true);
    setErrorMsg(null);
    try {
      // imageData may be a data URL — strip the prefix if present
      const raw = props.imageData!;
      const base64 = raw.includes(",") ? raw.split(",")[1] : raw;
      const result = await upscaleImage(base64, factor);
      props.onUpscaled(result);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMsg(msg);
      setTimeout(() => setErrorMsg(null), 4000);
    } finally {
      setUpscaling(false);
    }
  };

  return (
    <Show when={props.visible}>
      <div style={{ position: "relative", display: "inline-flex", "flex-direction": "column", gap: "4px" }}>
        {/* Button row */}
        <div style={{ display: "flex", gap: "1px" }}>
          {/* Main label — clicking opens menu */}
          <button
            disabled={disabled()}
            onClick={() => !disabled() && setShowMenu((v) => !v)}
            style={{
              display: "flex",
              "align-items": "center",
              gap: "6px",
              padding: "5px 10px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              "border-right": "none",
              "border-radius": "var(--radius) 0 0 var(--radius)",
              color: disabled() ? "var(--text-secondary)" : "var(--text-primary)",
              "font-size": "12px",
              "font-weight": "500",
              cursor: disabled() ? "not-allowed" : "pointer",
              opacity: disabled() ? "0.5" : "1",
              transition: "var(--transition)",
              "white-space": "nowrap",
            }}
          >
            <Show when={upscaling()} fallback={
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="15 3 21 3 21 9" />
                <polyline points="9 21 3 21 3 15" />
                <line x1="21" y1="3" x2="14" y2="10" />
                <line x1="3" y1="21" x2="10" y2="14" />
              </svg>
            }>
              {/* Spinner */}
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                style={{ animation: "spin 0.8s linear infinite" }}>
                <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
              </svg>
            </Show>
            {upscaling() ? "Upscaling…" : "Upscale"}
          </button>

          {/* Chevron toggle */}
          <button
            disabled={disabled()}
            onClick={() => !disabled() && setShowMenu((v) => !v)}
            style={{
              display: "flex",
              "align-items": "center",
              padding: "5px 7px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              "border-radius": "0 var(--radius) var(--radius) 0",
              color: disabled() ? "var(--text-secondary)" : "var(--text-primary)",
              "font-size": "12px",
              cursor: disabled() ? "not-allowed" : "pointer",
              opacity: disabled() ? "0.5" : "1",
              transition: "var(--transition)",
            }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>
        </div>

        {/* Dropdown */}
        <Show when={showMenu()}>
          <div
            style={{
              position: "absolute",
              top: "calc(100% + 4px)",
              left: "0",
              "z-index": "100",
              background: "var(--bg-secondary)",
              border: "1px solid var(--border)",
              "border-radius": "var(--radius)",
              overflow: "hidden",
              "min-width": "100%",
              "box-shadow": "0 4px 12px rgba(0,0,0,0.4)",
            }}
          >
            {([2, 4] as const).map((factor) => (
              <button
                onClick={() => handleUpscale(factor)}
                style={{
                  display: "block",
                  width: "100%",
                  padding: "7px 14px",
                  background: "none",
                  border: "none",
                  color: "var(--text-primary)",
                  "font-size": "13px",
                  "text-align": "left",
                  cursor: "pointer",
                  transition: "var(--transition)",
                  "white-space": "nowrap",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-tertiary)")}
                onMouseLeave={(e) => (e.currentTarget.style.background = "none")}
              >
                {factor}x
              </button>
            ))}
          </div>
          {/* Click-away overlay */}
          <div
            style={{ position: "fixed", inset: "0", "z-index": "99" }}
            onClick={() => setShowMenu(false)}
          />
        </Show>

        {/* Inline error */}
        <Show when={errorMsg()}>
          <div style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            left: "0",
            "white-space": "nowrap",
            padding: "4px 8px",
            background: "rgba(239,68,68,0.12)",
            border: "1px solid var(--error)",
            "border-radius": "var(--radius)",
            color: "var(--error)",
            "font-size": "11px",
            "z-index": "101",
          }}>
            {errorMsg()}
          </div>
        </Show>
      </div>

      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </Show>
  );
};

export default UpscaleButton;
