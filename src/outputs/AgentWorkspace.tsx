import { Markdown } from "../components/Markdown";
import type { ArtifactState } from "../native";
const toolName = { read_conversation: "Read conversation", draft_report: "Draft report", inspect_draft: "Check draft" };
export function AgentWorkspace({ state, busy, onExport }: { state: Extract<ArtifactState, { kind: "agent" }>; busy: boolean; onExport: () => void }) {
  return <section className="agent-reader" aria-label="Agent report">
    <div className="artifact-controls"><span aria-live="polite">{busy ? "Working…" : `Version ${state.revision}`}</span><button type="button" disabled={busy} onClick={onExport}>Export report</button></div>
    <p className="quiet-note">Local writing and synthesis from this saved conversation.</p>
    {state.content.steps.length > 0 && <details className="agent-steps"><summary>Work completed · {state.content.steps.length} steps</summary><ol>{state.content.steps.map((step, index) => <li key={index}><span>{toolName[step.tool]}</span><p>{step.summary}</p></li>)}</ol></details>}
    <article className="doc-paper" aria-label="Saved agent report"><h1>{state.content.title}</h1><Markdown text={state.content.markdown} allowImages={false} /></article>
  </section>;
}
