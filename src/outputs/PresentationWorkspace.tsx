import { useEffect, useState } from "react";
import { Markdown } from "../components/Markdown";

export type PresentationState = {
  kind: "presentation"; revision: number; requestCount: number;
  content: { title: string; slides: { title: string; body: string; notes: string }[] };
};
export function PresentationWorkspace({ state, busy, onExport, onEdit }: { state: PresentationState; busy: boolean; onExport: () => void; onEdit?: (content: PresentationState["content"]) => void }) {
  const [selected, setSelected] = useState(0);
  const [showNotes, setShowNotes] = useState(false);
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState(state.content);
  useEffect(() => { setEditing(false); setContent(state.content); }, [state.revision]);
  const count = state.content.slides.length;
  const index = Math.max(0, Math.min(selected, count - 1));
  useEffect(() => { setSelected((old) => Math.max(0, Math.min(old, count - 1))); }, [count]);
  const move = (value: number) => setSelected(Math.max(0, Math.min(value, count - 1)));
  const slide = state.content.slides[index];
  return <section className="presentation-reader" aria-label="Presentation" tabIndex={0} onKeyDown={(event) => {
    if ((event.target as HTMLElement).closest("input, textarea")) return;
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || event.nativeEvent.isComposing) return;
    if (event.key === "ArrowRight") { event.preventDefault(); move(index + 1); }
    if (event.key === "ArrowLeft") { event.preventDefault(); move(index - 1); }
    if (event.key === "Home") { event.preventDefault(); move(0); }
    if (event.key === "End") { event.preventDefault(); move(count - 1); }
  }}>
    <div className="artifact-controls"><span aria-live="polite">{busy ? "Updating…" : `Version ${state.revision}`}</span>{onEdit && !editing && <button type="button" disabled={busy || !slide} onClick={() => { setContent({ ...state.content, slides: state.content.slides.map((item) => ({ ...item })) }); setEditing(true); }}>Edit presentation</button>}<button type="button" disabled={busy || !slide || editing} onClick={onExport}>Export presentation</button></div>
    <div className="presentation-heading">{state.content.title}</div>
    {slide ? <>
      {editing ? <form className="artifact-editor" onSubmit={(event) => { event.preventDefault(); onEdit?.(content); }}><label>Presentation title<input disabled={busy} value={content.title} onChange={(event) => setContent((old) => ({ ...old, title: event.target.value }))} /></label>{([ ["title", "Slide title"], ["body", "Slide content"], ["notes", "Speaker notes"] ] as const).map(([field, label]) => <label key={field}>{label}<textarea disabled={busy} value={content.slides[index]?.[field] ?? ""} onChange={(event) => setContent((old) => ({ ...old, slides: old.slides.map((item, position) => position === index ? { ...item, [field]: event.target.value } : item) }))} /></label>)}<div className="artifact-controls"><button type="submit" disabled={busy || !content.title.trim() || content.slides.some((item) => !item.title.trim())}>Save edits</button><button type="button" disabled={busy} onClick={() => setEditing(false)}>Cancel edits</button></div></form> : <article className="slide-page" aria-label={`Slide ${index + 1}`}><h1>{slide.title}</h1><div className="slide-body"><Markdown text={slide.body} allowImages={false} /></div></article>}
      <div className="artifact-controls slide-navigation"><button type="button" aria-label="Previous slide" disabled={index === 0} onClick={() => move(index - 1)}>Previous</button><span role="status">{index + 1} / {count}</span><button type="button" aria-label="Next slide" disabled={index === count - 1} onClick={() => move(index + 1)}>Next</button>{!editing && slide.notes.trim() && <button type="button" aria-expanded={showNotes} aria-controls="presentation-notes" onClick={() => setShowNotes((old) => !old)}>{showNotes ? "Hide notes" : "Show notes"}</button>}</div>
      {!editing && showNotes && slide.notes.trim() && <aside className="slide-notes" id="presentation-notes" aria-label="Speaker notes"><Markdown text={slide.notes} allowImages={false} /></aside>}
    </> : <p className="artifact-empty">No slides are available.</p>}
  </section>;
}
