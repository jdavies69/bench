import { useEffect, useState } from "react";
import { Markdown } from "../components/Markdown";

export type DocumentState = {
  kind: "document"; revision: number; requestCount: number;
  content: { title: string; markdown: string };
};
export function DocumentWorkspace({ state, busy, onExport, onEdit }: { state: DocumentState; busy: boolean; onExport: () => void; onEdit?: (content: DocumentState["content"]) => void }) {
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState(state.content);
  useEffect(() => { setEditing(false); setContent(state.content); }, [state.revision]);
  return <>
    <div className="artifact-controls"><span aria-live="polite">{busy ? "Updating…" : `Version ${state.revision}`}</span>{onEdit && !editing && <button type="button" disabled={busy} onClick={() => { setContent({ ...state.content }); setEditing(true); }}>Edit document</button>}<button type="button" disabled={busy || editing} onClick={onExport}>Export document</button></div>
    {editing ? <form className="artifact-editor" onSubmit={(event) => { event.preventDefault(); onEdit?.(content); }}><label>Document title<input value={content.title} disabled={busy} onChange={(event) => setContent((old) => ({ ...old, title: event.target.value }))} /></label><label>Document content<textarea value={content.markdown} disabled={busy} onChange={(event) => setContent((old) => ({ ...old, markdown: event.target.value }))} /></label><div className="artifact-controls"><button type="submit" disabled={busy || !content.title.trim() || !content.markdown.trim()}>Save edits</button><button type="button" disabled={busy} onClick={() => setEditing(false)}>Cancel edits</button></div></form> : <article className="doc-paper" aria-label="Document"><h1>{state.content.title}</h1><Markdown text={state.content.markdown} allowImages={false} /></article>}
  </>;
}
