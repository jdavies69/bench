import { useEffect, useRef, useState } from "react";
import { native, type ApplicationExpression, type ApplicationValues, type ArtifactState } from "../native";
export function evaluateApplication(expression: ApplicationExpression, values: ApplicationValues, depth = 0, count = { nodes: 0 }): number {
  if (depth > 12 || ++count.nodes > 256) throw new Error("Calculation is unavailable");
  let value: number;
  if (expression.op === "constant") value = expression.value;
  else if (expression.op === "input") {
    const input = Object.prototype.hasOwnProperty.call(values, expression.id) ? values[expression.id] : undefined;
    if (typeof input !== "number" && typeof input !== "boolean") throw new Error("Input is unavailable");
    value = Number(input);
  } else {
    const left = evaluateApplication(expression.left, values, depth + 1, count), right = evaluateApplication(expression.right, values, depth + 1, count);
    switch (expression.op) {
      case "add": value = left + right; break;
      case "subtract": value = left - right; break;
      case "multiply": value = left * right; break;
      case "divide": value = left / right; break;
      case "min": value = Math.min(left, right); break;
      case "max": value = Math.max(left, right); break;
      default: throw new Error("Calculation is unavailable");
    }
  }
  if (!Number.isFinite(value) || Math.abs(value) > 1e12) throw new Error("Calculation is undefined or out of range");
  return value;
}
function result(expression: ApplicationExpression, values: ApplicationValues) {
  try { return new Intl.NumberFormat(undefined, { maximumFractionDigits: 6 }).format(evaluateApplication(expression, values)); }
  catch { return "Unavailable"; }
}
export function ApplicationWorkspace({ conversationId, state, busy, onExport }: { conversationId: string; state: Extract<ArtifactState, { kind: "application" }>; busy: boolean; onExport: () => void }) {
  const [values, setValues] = useState<ApplicationValues>({});
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saved, setSaved] = useState(false);
  const epoch = useRef(0); const working = useRef(false);
  const load = async () => {
    const attempt = ++epoch.current; setLoaded(false); setError("");
    try { const savedValues = await native.loadApplicationValues(conversationId); if (epoch.current === attempt) { setValues(savedValues); setLoaded(true); setDirty(false); setSaved(false); } }
    catch { if (epoch.current === attempt) setError("Couldn’t load saved inputs. Try again before making changes."); }
  };
  useEffect(() => { void load(); return () => { epoch.current++; }; }, [conversationId, state.revision]);
  const save = async () => {
    if (!loaded || busy || working.current || !dirty) return;
    working.current = true; const attempt = epoch.current; setSaving(true); setError("");
    try { const savedValues = await native.saveApplicationValues(conversationId, state.revision, values); if (epoch.current === attempt) { setValues(savedValues); setDirty(false); setSaved(true); } }
    catch { if (epoch.current === attempt) setError("Couldn’t save inputs. Your changes are still here; try again."); }
    finally { if (epoch.current === attempt) { working.current = false; setSaving(false); } }
  };
  const change = (id: string, value: string | number | boolean) => { setValues((old) => ({ ...old, [id]: value })); setDirty(true); setSaved(false); };
  return <section className="application-reader" aria-label="Application">
    <div className="artifact-controls"><span>{busy ? "Updating…" : `Version ${state.revision}`}</span><button type="button" disabled={busy || saving} onClick={onExport}>Export application</button></div>
    <div className="application-page"><h1>{state.content.title}</h1><p>{state.content.description}</p>
      {!loaded && !error && <p role="status">Loading saved inputs…</p>}
      {loaded && <><form className="application-fields" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        {state.content.fields.map((field) => <label key={field.id}>{field.label}{field.type === "toggle" ? <input type="checkbox" disabled={busy || saving} checked={values[field.id] === true} onChange={(event) => change(field.id, event.target.checked)} /> : field.type === "select" ? <select disabled={busy || saving} value={String(values[field.id] ?? "")} onChange={(event) => change(field.id, event.target.value)}>{field.options?.map((option, index) => <option key={index} value={option}>{option}</option>)}</select> : <input type={field.type === "number" ? "number" : "text"} step={field.type === "number" ? "any" : undefined} maxLength={field.type === "text" ? 2000 : undefined} disabled={busy || saving} value={String(values[field.id] ?? "")} onChange={(event) => change(field.id, field.type === "number" && event.target.value !== "" ? Number(event.target.value) : event.target.value)} />}</label>)}
        <button type="submit" disabled={busy || saving || !dirty}>{saving ? "Saving…" : "Save inputs"}</button>
      </form><dl className="application-results">{state.content.outputs.map((output, index) => <div key={index}><dt>{output.label}</dt><dd><output>{result(output.expression, values)}</output></dd></div>)}</dl>
      <p className="quiet-note">Save inputs to keep them when you reopen this application.</p>{saved && <p role="status" className="quiet-note">Inputs saved.</p>}</>}
      {error && <p role="alert" className="usage-error">{error}</p>}{!loaded && error && <button type="button" onClick={() => void load()}>Retry saved inputs</button>}
    </div>
  </section>;
}
