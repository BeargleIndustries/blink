import { Component, Show, createSignal, createEffect, onMount, onCleanup } from "solid-js";
import MaskCanvas, { MaskCanvasAPI } from "./MaskCanvas";
import InpaintToolbar from "./InpaintToolbar";

interface ImageCanvasProps {
  imageData: string | null;
  generating: boolean;
  onImageDrop: (imageData: string) => void;
  onClearImage: () => void;
  inputImage: string | null;
  previewImage?: string | null;
  ref?: (api: ImageCanvasAPI) => void;
}

export interface ImageCanvasAPI {
  getMask: () => number[] | null;
  clearMask: () => void;
}

const ImageCanvas: Component<ImageCanvasProps> = (props) => {
  const [dragOver, setDragOver] = createSignal(false);
  const [brushSize, setBrushSize] = createSignal(30);
  const [isEraser, setIsEraser] = createSignal(false);
  const [hasMask, setHasMask] = createSignal(false);

  // Natural image dimensions — updated when input image loads
  const [imgNaturalWidth, setImgNaturalWidth] = createSignal(512);
  const [imgNaturalHeight, setImgNaturalHeight] = createSignal(512);

  // Measured container dimensions (responsive)
  const [containerW, setContainerW] = createSignal(512);
  const [containerH, setContainerH] = createSignal(512);

  let containerRef: HTMLDivElement | undefined;
  let maskApi: MaskCanvasAPI | undefined;
  let resizeObserver: ResizeObserver | undefined;

  onMount(() => {
    if (containerRef) {
      setContainerW(containerRef.clientWidth);
      setContainerH(containerRef.clientHeight);
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          setContainerW(entry.contentRect.width);
          setContainerH(entry.contentRect.height);
        }
      });
      resizeObserver.observe(containerRef);
    }
  });

  onCleanup(() => {
    resizeObserver?.disconnect();
  });

  // Clear mask when a new input image is loaded
  createEffect(() => {
    const img = props.inputImage;
    if (img) {
      maskApi?.clearMask();
      setHasMask(false);
    }
  });

  // Expose API to parent
  onMount(() => {
    if (props.ref) {
      props.ref({
        getMask: () => maskApi?.exportMask() ?? null,
        clearMask: () => maskApi?.clearMask(),
      });
    }
  });

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

  const handleImgLoad = (e: Event) => {
    const img = e.currentTarget as HTMLImageElement;
    setImgNaturalWidth(img.naturalWidth);
    setImgNaturalHeight(img.naturalHeight);
  };

  const handleClearImage = () => {
    maskApi?.clearMask();
    setHasMask(false);
    props.onClearImage();
  };

  return (
    <div style={{ display: "flex", "flex-direction": "column", "align-items": "center", gap: "8px" }}>
      <div
        ref={containerRef}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        style={{
          width: "512px",
          "max-width": "100%",
          "max-height": "calc(100vh - 280px)",
          "min-height": "256px",
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
            onLoad={handleImgLoad}
            style={{
              "max-width": "100%",
              "max-height": "100%",
              "object-fit": "contain",
              opacity: "0.5",
            }}
            alt="Input image"
          />
          <MaskCanvas
            containerWidth={containerW()}
            containerHeight={containerH()}
            imgNaturalWidth={imgNaturalWidth()}
            imgNaturalHeight={imgNaturalHeight()}
            brushSize={brushSize()}
            isEraser={isEraser()}
            isActive={true}
            onMaskChange={setHasMask}
            ref={(api) => { maskApi = api; }}
          />
          <button
            onClick={handleClearImage}
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
              "z-index": "10",
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

        <Show when={props.generating && props.previewImage && !props.inputImage}>
          <img
            src={props.previewImage!.startsWith("data:") ? props.previewImage! : `data:image/jpeg;base64,${props.previewImage}`}
            style={{
              "max-width": "100%",
              "max-height": "100%",
              "object-fit": "contain",
              opacity: "0.85",
            }}
            alt="Preview"
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
            <div style={{ "font-size": "12px", "margin-top": "8px", color: "var(--text-muted)" }}>
              Drop an image here for img2img, or
            </div>
            <button
              onClick={() => {
                const input = document.createElement("input");
                input.type = "file";
                input.accept = "image/*";
                input.onchange = () => {
                  const file = input.files?.[0];
                  if (file) {
                    const reader = new FileReader();
                    reader.onload = () => {
                      props.onImageDrop(reader.result as string);
                    };
                    reader.readAsDataURL(file);
                  }
                };
                input.click();
              }}
              style={{
                "margin-top": "8px",
                padding: "6px 16px",
                background: "var(--bg-tertiary)",
                border: "1px solid var(--border)",
                "border-radius": "var(--radius)",
                color: "var(--text-secondary)",
                cursor: "pointer",
                "font-size": "12px",
                transition: "var(--transition)",
              }}
            >
              Browse for image
            </button>
          </div>
        </Show>

        <Show when={props.generating && !props.imageData && !props.inputImage}>
          <div style={{
            "text-align": "center",
            color: "var(--text-secondary)",
          }}>
            <div style={{ "font-size": "24px" }}>Generating...</div>
          </div>
        </Show>
      </div>

      <InpaintToolbar
        visible={!!props.inputImage}
        brushSize={brushSize()}
        isEraser={isEraser()}
        onBrushSizeChange={setBrushSize}
        onModeChange={setIsEraser}
        onClearMask={() => maskApi?.clearMask()}
      />
    </div>
  );
};

export default ImageCanvas;
