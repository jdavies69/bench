import { useEffect, useState } from "react";
import type { WebsiteRevision, WebsiteState } from "../native";
import { buildPreviewDocument } from "./preview";

export function WebsiteWorkspace({ state, revisions, busy, onRestore }: {
  state: WebsiteState; revisions: WebsiteRevision[]; busy: boolean; onRestore: (revision: number) => void;
}) {
  const [page, setPage] = useState(state.pages[0]?.path ?? "");
  useEffect(() => { if (!state.pages.some((item) => item.path === page)) setPage(state.pages[0]?.path ?? ""); }, [state, page]);
  const html = state.pages.find((item) => item.path === page)?.html ?? state.pages[0]?.html ?? "";
  return <>
    <div className="preview-toolbar">
      <span aria-live="polite">{busy ? "Updating…" : "Preview"}</span>
      {state.pages.length > 1 && <select aria-label="Preview page" value={page} onChange={(event) => setPage(event.target.value)}>{state.pages.map((item) => <option key={item.path} value={item.path}>{item.path}</option>)}</select>}
      <details className="website-versions" key={state.revision}>
        <summary aria-label="Website versions">Version {state.revision}<span aria-hidden="true">⌄</span></summary>
        <div className="version-list">{[...revisions].reverse().map((revision) => <button key={revision.revision} type="button" disabled={busy || revision.current} onClick={() => onRestore(revision.revision)}>{revision.current ? `Version ${revision.revision} · Current` : `Restore version ${revision.revision}`}</button>)}</div>
      </details>
    </div>
    <iframe title="Website preview" sandbox="" referrerPolicy="no-referrer" className="website-preview" srcDoc={buildPreviewDocument(html, state.css)} />
  </>;
}
