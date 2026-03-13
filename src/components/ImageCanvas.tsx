import { Component, Show, createSignal } from "solid-js";

interface ImageCanvasProps {
  imageData: string | null;
  generating: boolean;
  onImageDrop: (imageData: string) => void;
  onClearImage: () => void;
  inputImage: string | null;
}

const ImageCanvas: Component<ImageCanvasProps> = (props) => {
  const [dragOver, setDragOver] = createSignal(false);

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  };

  const handleDragLeave = () => setDragOver(false);

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      const file = files[0];
      if (file.type.startsWith("image/")) {
        const reader = new FileReader();
        reader.onload = () => {
          props.onImageDrop(reader.result as string);
        };
        reader.readAsDataURL(file);
      }
    }
  };

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      style={{
        width: "512px",
        height: "512px",
        "max-width": "100%",
        "aspect-ratio": "1",
        background: "var(--bg-secondary)",
        "border-radius": "var(--radius)",
        border: `2px ${dragOver() ? "solid var(--accent)" : "dashed var(--border)"}`,
        display: "flex",
        "align-items": "center",
        "justify-content": "center",
        position: "relative",
        overflow: "hidden",
        transition: "border 0.2s",
      }}
    >
      <Show when={props.inputImage}>
        <img
          src={props.inputImage!}
          style={{
            "max-width": "100%",
            "max-height": "100%",
            "object-fit": "contain",
            opacity: "0.5",
          }}
          alt="Input image"
        />
        <button
          onClick={props.onClearImage}
          style={{
            position: "absolute",
            top: "8px",
            right: "8px",
            background: "rgba(0,0,0,0.7)",
            border: "none",
            color: "white",
            width: "28px",
            height: "28px",
            "border-radius": "50%",
            cursor: "pointer",
            "font-size": "14px",
          }}
        >
          X
        </button>
      </Show>

      <Show when={props.imageData && !props.inputImage}>
        <img
          src={
            props.imageData!.startsWith("data:") || props.imageData!.startsWith("asset://")
              ? props.imageData!
              : `data:image/png;base64,${props.imageData}`
          }
          style={{
            "max-width": "100%",
            "max-height": "100%",
            "object-fit": "contain",
          }}
          alt="Generated image"
        />
      </Show>

      <Show when={!props.imageData && !props.inputImage && !props.generating}>
        <div style={{
          "text-align": "center",
          color: "var(--text-secondary)",
          padding: "20px",
        }}>
          <div style={{ "font-size": "48px", "margin-bottom": "12px", opacity: "0.3" }}>
            &#127912;
          </div>
          <div>Generated image will appear here</div>
          <div style={{ "font-size": "12px", "margin-top": "8px" }}>
            Drop an image here for img2img
          </div>
        </div>
      </Show>

      <Show when={props.generating && !props.imageData}>
        <div style={{
          "text-align": "center",
          color: "var(--text-secondary)",
        }}>
          <div style={{ "font-size": "24px" }}>Generating...</div>
        </div>
      </Show>
    </div>
  );
};

export default ImageCanvas;
