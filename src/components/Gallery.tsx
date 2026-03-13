import { Component, For, Show, createSignal } from "solid-js";
import type { GalleryItem } from "../lib/types";
import { deleteGalleryItem } from "../lib/tauri-api";

interface GalleryProps {
  items: GalleryItem[];
  onSelect: (item: GalleryItem) => void;
  onDelete: (itemId: string) => void;
}

const Gallery: Component<GalleryProps> = (props) => {
  const [hoveredId, setHoveredId] = createSignal<string | null>(null);

  const handleDelete = async (e: MouseEvent, itemId: string) => {
    e.stopPropagation();
    try {
      await deleteGalleryItem(itemId);
      props.onDelete(itemId);
    } catch (err) {
      console.error("Failed to delete gallery item:", err);
    }
  };

  return (
    <div style={{
      display: "flex",
      "align-items": "center",
      gap: "6px",
      "overflow-x": "auto",
      padding: "4px 0",
      "min-height": "64px",
    }}>
      <Show
        when={props.items.length > 0}
        fallback={
          <span style={{
            color: "var(--text-secondary)",
            "font-size": "12px",
            padding: "0 8px",
            opacity: "0.5",
          }}>
            Generated images will appear here
          </span>
        }
      >
        <For each={props.items}>
          {(item) => (
            <div
              onClick={() => props.onSelect(item)}
              onMouseEnter={() => setHoveredId(item.id)}
              onMouseLeave={() => setHoveredId(null)}
              style={{
                width: "56px",
                height: "56px",
                "min-width": "56px",
                "border-radius": "4px",
                overflow: "hidden",
                cursor: "pointer",
                border: hoveredId() === item.id
                  ? "1px solid var(--accent)"
                  : "1px solid var(--border)",
                position: "relative",
                transition: "border-color 0.15s",
                "flex-shrink": "0",
              }}
              title={item.prompt}
            >
              <img
                src={`asset://localhost/${item.thumbnail_path}`}
                style={{
                  width: "100%",
                  height: "100%",
                  "object-fit": "cover",
                  display: "block",
                }}
                alt={item.prompt}
                loading="lazy"
                onError={(e) => {
                  // Fallback: hide broken image
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
              <Show when={hoveredId() === item.id}>
                <button
                  onClick={(e) => handleDelete(e, item.id)}
                  style={{
                    position: "absolute",
                    top: "2px",
                    right: "2px",
                    width: "16px",
                    height: "16px",
                    "border-radius": "50%",
                    background: "rgba(0,0,0,0.75)",
                    border: "none",
                    color: "white",
                    "font-size": "9px",
                    cursor: "pointer",
                    display: "flex",
                    "align-items": "center",
                    "justify-content": "center",
                    "line-height": "1",
                    padding: "0",
                  }}
                  title="Delete"
                >
                  &#x2715;
                </button>
              </Show>
            </div>
          )}
        </For>
      </Show>
    </div>
  );
};

export default Gallery;
