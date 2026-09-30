import { useState } from "react";
import type { ArtifactState } from "../native";
export function MediaWorkspace({ state, busy, onExport }: { state: Extract<ArtifactState, { kind: "image" | "voice" }>; busy: boolean; onExport: () => void }) {
  const [zoom, setZoom] = useState(false);
  const { mimeType, dataBase64, prompt } = state.content;
  const valid = state.kind === "image" ? ["image/png", "image/jpeg", "image/webp"].includes(mimeType) : mimeType === "audio/mpeg";
  // Rust validates decoded bytes and immutable storage. Presentation also
  // rejects remote URLs or active document MIME types at this boundary.
  const source = valid && /^[A-Za-z0-9+/]+={0,2}$/.test(dataBase64) ? `data:${mimeType};base64,${dataBase64}` : null;
  return <section className="media-reader" aria-label={`${state.kind === "image" ? "Image" : "Voice"} output`}>
    <div className="artifact-controls"><span aria-live="polite">{busy ? "Updating…" : `Version ${state.revision}`}</span>{state.kind === "image" && source && <button type="button" aria-pressed={zoom} onClick={() => setZoom((old) => !old)}>{zoom ? "Fit image" : "Actual size"}</button>}<button type="button" disabled={busy || !source} onClick={onExport}>Export {state.kind === "image" ? "image" : "audio"}</button></div>
    {source ? state.kind === "image" ? <div className={`image-canvas ${zoom ? "image-actual-size" : ""}`}><img src={source} alt={prompt || "Generated image"} /></div> : <div className="voice-player"><p className="voice-script">{prompt}</p><audio key={state.revision} controls preload="none" src={source} aria-label="Generated voice" /></div> : <p className="artifact-empty" role="alert">This media could not be displayed. Your saved output is still here.</p>}
  </section>;
}
