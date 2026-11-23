import { useEffect, useRef, useState } from "react";
import { native, type Project, type ScopeChange, type ScopeDecision, type ScopeItem, type ScopeItemChange, type ScopeState } from "../native";

const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

/** SQLite stores UTC as "YYYY-MM-DD HH:MM:SS". */
export function ago(timestamp: string, now = Date.now()): string {
  const then = Date.parse(`${timestamp.replace(" ", "T")}Z`);
  if (Number.isNaN(then)) return "";
  const minutes = Math.max(0, Math.floor((now - then) / 60_000));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  if (minutes < 1440) return `${Math.floor(minutes / 60)} h ago`;
  return `${Math.floor(minutes / 1440)} d ago`;
}

type ChangeCopy = { label: string; actions: [string, ScopeDecision][] };
function changeCopy(change: ScopeChange): ChangeCopy {
  if (change.kind === "complete") return { label: "Looks done", actions: [["Mark done", "accept"], ["Not yet", "dismiss"]] };
  if (change.kind === "reconsider") return { label: "Still needed?", actions: [["Keep", "dismiss"], ["Later", "later"], ["Drop", "accept"]] };
  // An idea the outcome does not need is parked by default; adding it to scope is a deliberate choice.
  return change.fit === "needed"
    ? { label: "Missing from scope", actions: [["Add to scope", "accept"], ["Later", "later"], ["Dismiss", "dismiss"]] }
    : { label: "New idea", actions: [["Later", "later"], ["Add to scope", "accept"], ["Dismiss", "dismiss"]] };
}

function Meter({ label, done, total }: { label: string; done: number; total: number }) {
  const percent = total === 0 ? 0 : Math.round((done / total) * 100);
  return <div className="scope-meter" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}><span style={{ width: `${percent}%` }} /></div>;
}

export function ScopePanel({ project, busy, onClose, onStart, onOpen, onChanged, onError }: {
  project: Project; busy: boolean; onClose: () => void;
  onStart: (item: ScopeItem, prompt: string) => void; onOpen: (conversationId: string) => void;
  onChanged: () => void; onError: (message: string) => void;
}) {
  const [scope, setScope] = useState<ScopeState | null | undefined>(undefined);
  const [unavailable, setUnavailable] = useState(false);
  const [working, setWorking] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [outcome, setOutcome] = useState("");
  const [editingOutcome, setEditingOutcome] = useState(false);
  const [newItem, setNewItem] = useState("");
  const live = useRef(true);
  const updatingRef = useRef(false);

  const show = (next: ScopeState) => { if (!live.current) return; setScope(next); onChanged(); };
  const run = async (action: () => Promise<ScopeState>) => {
    if (working) return false;
    setWorking(true);
    try { show(await action()); onError(""); return true; }
    catch (error) { if (live.current) onError(errorText(error)); return false; }
    finally { if (live.current) setWorking(false); }
  };
  const update = async (force: boolean) => {
    if (updatingRef.current) return;
    updatingRef.current = true; setUpdating(true);
    try { show(await native.reconcileScope(project.id, force)); if (force) onError(""); }
    catch (error) {
      if (!live.current) return;
      // A failed automatic check stays quiet here; the status line reports it.
      if (force) onError(errorText(error));
      const current = await native.loadScope(project.id).catch(() => null);
      if (live.current && current) setScope(current);
    } finally { updatingRef.current = false; if (live.current) setUpdating(false); }
  };
  useEffect(() => {
    live.current = true;
    native.loadScope(project.id).then((loaded) => {
      if (!live.current) return;
      setScope(loaded);
      // Catch up on work finished while this page was closed. A failed check waits for the user.
      if (loaded?.stale && loaded.autoUpdate && loaded.lastRunStatus !== "failed") void update(false);
    }).catch((error) => { if (live.current) { setUnavailable(true); onError(errorText(error)); } });
    return () => { live.current = false; };
  }, [project.id]);

  const start = async (item: ScopeItem) => {
    if (busy || working) return;
    setWorking(true);
    try { onStart(item, await native.scopeSessionPrompt(item.id)); }
    catch (error) { if (live.current) onError(errorText(error)); }
    finally { if (live.current) setWorking(false); }
  };
  const change = (item: ScopeItem, next: ScopeItemChange) => void run(() => native.updateScopeItem(item.id, next));
  const saveOutcome = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!outcome.trim()) return;
    if (await run(() => native.setScopeOutcome(project.id, outcome))) { setEditingOutcome(false); setOutcome(""); }
  };
  const addItem = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!newItem.trim()) return;
    if (await run(() => native.addScopeItem(project.id, newItem, "scope"))) setNewItem("");
  };

  const top = <div className="panel-top"><h1>{project.name}</h1><button className="icon-button" onClick={onClose} aria-label="Close scope"><svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" aria-hidden="true"><path d="M5 5 19 19M19 5 5 19" /></svg></button></div>;
  const outcomeForm = (label: string) => <form onSubmit={(event) => void saveOutcome(event)}><div className="field-row"><input autoFocus value={outcome} maxLength={280} onChange={(event) => setOutcome(event.target.value)} placeholder="One sentence describing the finished project" aria-label="Outcome" /><button type="submit" disabled={working || !outcome.trim()}>{label}</button></div></form>;

  if (unavailable) return <section className="utility-panel scope-panel" aria-label={`${project.name} scope`}>{top}<p className="quiet-note">Scope could not be loaded.</p></section>;
  if (scope === undefined) return <section className="utility-panel scope-panel" aria-label={`${project.name} scope`}>{top}</section>;
  if (scope === null) return <section className="utility-panel scope-panel" aria-label={`${project.name} scope`}>{top}
    <section className="settings-section"><h2>What does done look like?</h2>{outcomeForm("Set outcome")}<p className="quiet-note scope-note">Everything in this project is tracked against this outcome. You decide what enters the list.</p></section>
  </section>;

  const { progress } = scope;
  const open = scope.items.filter((item) => item.bucket === "scope" && item.status !== "done");
  const finished = scope.items.filter((item) => item.bucket === "scope" && item.status === "done");
  const later = scope.items.filter((item) => item.bucket === "later");
  const status = updating ? "Updating…" : scope.lastRunStatus === "failed" ? "Last update failed" : scope.stale ? "New work not reviewed yet" : scope.lastRunAt ? `Updated ${ago(scope.lastRunAt)}` : "Not updated yet";
  const sessions = (item: ScopeItem) => item.sessions.length > 0 && <div className="scope-sessions">{item.sessions.map((session) => <button key={session.conversationId} type="button" className="text-button" onClick={() => onOpen(session.conversationId)}>{session.title}</button>)}</div>;

  return <section className="utility-panel scope-panel" aria-label={`${project.name} scope`}>{top}
    <section className="settings-section"><h2>Outcome</h2>
      {editingOutcome ? outcomeForm("Save") : <p className="scope-outcome">{scope.outcome} <button type="button" className="text-button" onClick={() => { setOutcome(scope.outcome); setEditingOutcome(true); }}>Edit</button></p>}
    </section>
    <section className="settings-section"><h2>Progress</h2>
      <Meter label="Scope complete" done={progress.done} total={progress.inScope} />
      <div className="scope-figures"><span>{progress.done} of {progress.inScope} done</span><span>{progress.percent}%</span></div>
      <p className="scope-week">Last 7 days: {progress.addedWeek} added · {progress.doneWeek} done{progress.later > 0 ? ` · ${progress.later} parked for later` : ""}</p>
      {progress.areas.length > 1 && <div className="scope-areas">{progress.areas.map((area) => <div className="scope-area" key={area.name}><span>{area.name}</span><Meter label={`${area.name} complete`} done={area.done} total={area.total} /><small>{area.done}/{area.total}</small></div>)}</div>}
    </section>
    <section className="settings-section" aria-label="Changed">
      <div className="scope-section-head"><h2>Changed{scope.changes.length > 0 ? ` · ${scope.changes.length}` : ""}</h2><span className="scope-run"><span>{status}</span><button type="button" className="text-button" disabled={updating} onClick={() => void update(true)}>Update now</button></span></div>
      {scope.changes.length === 0 && <p className="quiet-note scope-note">Nothing to review.</p>}
      {scope.changes.map((item) => { const copy = changeCopy(item); return <div className="scope-change" key={item.id}>
        <div className="scope-change-kind">{copy.label}</div>
        <div className="scope-change-title">{item.title}</div>
        <p>{item.reason}</p>
        {item.conversationId && item.conversationTitle && <button type="button" className="text-button scope-source" onClick={() => onOpen(item.conversationId!)}>From “{item.conversationTitle}”</button>}
        <div className="action-review-controls">{copy.actions.map(([label, decision]) => <button key={decision} type="button" disabled={working} aria-label={`${label}: ${item.title}`} onClick={() => void run(() => native.resolveScopeChange(item.id, decision))}>{label}</button>)}</div>
      </div>; })}
      <div className="web-search-row"><div><div className="setting-label">Keep this updated</div><div className="advanced-state">Reviews new work after each response, using your connected model. Suggestions wait here for you.</div></div><button type="button" className="setting-switch" role="switch" aria-label="Keep scope updated" aria-checked={scope.autoUpdate} disabled={working} onClick={() => void run(() => native.setScopeAutoUpdate(project.id, !scope.autoUpdate))}><span /></button></div>
    </section>
    <section className="settings-section"><h2>To do</h2>
      {open.map((item, index) => <div className="scope-item" key={item.id}>
        <input type="checkbox" checked={false} disabled={working} aria-label={`Done: ${item.title}`} onChange={() => change(item, { status: "done" })} />
        <div className="scope-item-body"><span>{item.title}</span>{item.area && <span className="scope-tag">{item.area}</span>}{item.status === "doing" && <span className="scope-tag">In progress</span>}{item.doneWhen && <small>Done when: {item.doneWhen}</small>}{sessions(item)}</div>
        <div className="scope-item-actions"><button type="button" className="text-button" disabled={working} aria-label={`Later: ${item.title}`} onClick={() => change(item, { bucket: "later" })}>Later</button><button type="button" className={`scope-start ${index === 0 ? "scope-start-next" : ""}`} disabled={busy || working} aria-label={`Start: ${item.title}`} onClick={() => void start(item)}>Start</button></div>
      </div>)}
      {scope.items.length === 0 && scope.changes.length === 0 && <div className="scope-draft"><button type="button" className="setup-help" disabled={updating} onClick={() => void update(true)}>Draft a list from the outcome</button><p className="quiet-note scope-note">Uses one request to your connected model. Nothing joins the list until you accept it.</p></div>}
      <form className="scope-add" onSubmit={(event) => void addItem(event)}><div className="field-row"><input value={newItem} maxLength={120} onChange={(event) => setNewItem(event.target.value)} placeholder="Add an item" aria-label="Add an item" /><button type="submit" disabled={working || !newItem.trim()}>Add</button></div></form>
      {finished.length > 0 && <details className="scope-group"><summary>Done · {finished.length}</summary>{finished.map((item) => <div className="scope-item scope-item-done" key={item.id}>
        <input type="checkbox" checked disabled={working} aria-label={`Done: ${item.title}`} onChange={() => change(item, { status: "todo" })} />
        <div className="scope-item-body"><span>{item.title}</span>{sessions(item)}</div>
      </div>)}</details>}
      {later.length > 0 && <details className="scope-group"><summary>Later · {later.length}</summary>{later.map((item) => <div className="scope-item" key={item.id}>
        <div className="scope-item-body"><span>{item.title}</span>{item.area && <span className="scope-tag">{item.area}</span>}</div>
        <div className="scope-item-actions"><button type="button" className="text-button" disabled={working} aria-label={`Drop: ${item.title}`} onClick={() => change(item, { bucket: "dropped" })}>Drop</button><button type="button" className="text-button" disabled={working} aria-label={`Move to scope: ${item.title}`} onClick={() => change(item, { bucket: "scope" })}>Move to scope</button></div>
      </div>)}</details>}
    </section>
  </section>;
}
