import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Markdown } from "./components/Markdown";
import { OutputPicker } from "./components/OutputPicker";
import { OpenRouterSetup } from "./components/OpenRouterSetup";
import { OpenRouterUsage } from "./components/OpenRouterUsage";
import { workspaceModules } from "./outputs/registry";
import { native, type ActionReview, type OutputDefinition, type WebsiteRevision, type ToolActivity, type ApprovalBehavior, type Conversation, type ExecutionBehavior, type Message, type OutputType, type Project, type ProviderId, type Snapshot, type WebsiteState } from "./native";
import "./App.css";

type IconName = "mark" | "plus" | "search" | "folder" | "chevron" | "arrow" | "panel" | "close";
const outputName = (type: OutputType) => type.charAt(0).toUpperCase() + type.slice(1);
type PendingReview = { review: ActionReview; operation: "generate" | "restore"; targetRevision?: number };
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
function beginWindowDrag(event: React.MouseEvent<HTMLElement>) {
  if (event.button !== 0 || !isTauri()) return;
  void getCurrentWindow().startDragging().catch((error) => console.error("Could not drag Bench window", error));
}
function Icon({ name, size = 19 }: { name: IconName; size?: number }) {
  const common = { width: size, height: size, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.65, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true as const };
  const paths: Record<IconName, React.ReactNode> = {
    mark: <path d="M7 3.5v17m0-17h6.2c3.1 0 4.9 1.6 4.9 4.2s-1.8 4.1-4.9 4.1H7m6.2 0c3.5 0 5.4 1.6 5.4 4.3s-1.9 4.4-5.4 4.4H7" />,
    plus: <path d="M12 4v16M4 12h16" />,
    search: <><circle cx="10.8" cy="10.8" r="6.5" /><path d="m16 16 5 5" /></>,
    folder: <path d="M3 7.5a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2-2z" />,
    chevron: <path d="m7 10 5 5 5-5" />,
    arrow: <><path d="M12 20V4" /><path d="m5 11 7-7 7 7" /></>,
    panel: <><rect x="3" y="3" width="18" height="18" rx="3" /><path d="M9 3v18" /></>,
    close: <path d="M5 5 19 19M19 5 5 19" />,
  };
  return <svg {...common}>{paths[name]}</svg>;
}
function Composer({ value, onChange, onSend, busy, outputType, setOutputType, centered, definitions }: {
  value: string; onChange: (value: string) => void; onSend: () => void; busy: boolean;
  outputType: OutputType; setOutputType: (value: OutputType) => void; centered: boolean; definitions: OutputDefinition[];
}) {
  const textarea = useRef<HTMLTextAreaElement>(null);
  useEffect(() => { if (textarea.current) { textarea.current.style.height = "auto"; textarea.current.style.height = `${Math.min(textarea.current.scrollHeight, 160)}px`; } }, [value]);
  return <form className={`composer ${centered ? "composer-centered" : ""}`} onSubmit={(event) => { event.preventDefault(); onSend(); }}>
    <button type="button" className="composer-add" title="Attachments are coming later" aria-label="Add attachment (coming later)" disabled><Icon name="plus" size={22} /></button>
    <textarea ref={textarea} rows={1} value={value} onChange={(event) => onChange(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && event.keyCode !== 229) { event.preventDefault(); onSend(); } }} placeholder="Describe what you want to make..." aria-label="Message" />
    <OutputPicker value={outputType} onChange={setOutputType} definitions={definitions} />
    <button className="send-button" type="submit" disabled={!value.trim() || busy} aria-label="Send message"><Icon name="arrow" size={21} /></button>
  </form>;
}
function responseError(error: unknown): string {
  const raw = errorText(error);
  const message = raw.toLowerCase();
  if (/rejected the api key|is rate limiting requests|took too long to respond|is temporarily unavailable|stopped before finishing|could not reach|web search isn't connected|web search is unavailable|web search timed out|web search found no usable results|could not process this request|could not complete the request/i.test(raw)) return raw;
  if (message.includes("401") || message.includes("403") || message.includes("invalid api key") || message.includes("unauthorized")) return "Check the provider key in Settings.";
  if (message.includes("429") || message.includes("rate limit")) return "Rate limit reached. Try again shortly.";
  if (message.includes("timeout") || message.includes("timed out")) return "The response timed out.";
  if (message.includes("network") || message.includes("connection") || message.includes("offline")) return "Connection lost.";
  if (message.includes("empty")) return "The provider returned an empty response.";
  if (message.includes("malformed") || message.includes("invalid response")) return "The provider returned an invalid response.";
  if (message.includes("database") || message.includes("sqlite")) return "Couldn’t save the response.";
  return "Couldn’t complete the response.";
}
function SettingsView({ snapshot, onClose, onSaveBehaviors, onRefresh, onError, onSetupOpenRouter }: {
  snapshot: Snapshot; onClose: () => void; onSaveBehaviors: (execution: ExecutionBehavior, approval: ApprovalBehavior) => void;
  onRefresh: () => Promise<void>; onError: (message: string) => void;
  onSetupOpenRouter: () => void;
}) {
  const [selectedProvider, setSelectedProvider] = useState<ProviderId | null>(() => snapshot.providers.find((item) => item.id === "openrouter" && item.keySource === "none")?.id ?? null);
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => { let live = true; if (isTauri()) void getVersion().then((value) => { if (live) setVersion(value); }).catch(() => {}); return () => { live = false; }; }, []);
  const [models, setModels] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [connectionRevision, setConnectionRevision] = useState(0);
  const keyInput = useRef<HTMLInputElement>(null);
  useEffect(() => { if (keyInput.current) keyInput.current.value = ""; }, [selectedProvider]);
  const perform = async (action: () => Promise<unknown>, resetUsage = false) => {
    setSaving(true);
    try { await action(); await onRefresh(); if (resetUsage) setConnectionRevision((old) => old + 1); onError(""); } catch (error) { onError(errorText(error)); } finally { setSaving(false); }
  };
  return <section className="utility-panel settings-panel">
    <div className="panel-top"><h1>Settings</h1><button className="icon-button" onClick={onClose} aria-label="Close settings"><Icon name="close" /></button></div>
    <section className="settings-section"><h2>Connections</h2><p className="quiet-note">Connect OpenRouter to use Chat and Website. OpenRouter bills you directly.</p><div className="connection-list">{snapshot.providers.filter((item) => item.id === "openrouter").map((item) => <div className="connection" key={item.id}>
      <button className="connection-row" type="button" aria-expanded={selectedProvider === item.id} onClick={() => setSelectedProvider(selectedProvider === item.id ? null : item.id)}><span className="connection-name">{item.label}{snapshot.settings.modelProvider === item.id && <span className="active-dot" title="Active provider" />}</span><span className="connection-state">{item.keySource === "none" ? "Not connected" : "Connected"}</span><Icon name="chevron" size={16} /></button>
      {selectedProvider === item.id && <div className="connection-detail">
        {item.id === "openrouter" && <button type="button" className="setup-help" disabled={saving} onClick={onSetupOpenRouter}>Connect OpenRouter</button>}
        <form onSubmit={(event) => { event.preventDefault(); const key = keyInput.current?.value ?? ""; if (!key.trim()) return; void perform(async () => { await native.saveProviderKey(item.id, key); if (keyInput.current) keyInput.current.value = ""; }, item.id === "openrouter"); }}><label htmlFor="provider-key">API key</label><div className="field-row"><input ref={keyInput} id="provider-key" type="password" autoComplete="off" spellCheck={false} placeholder={item.keySource === "keychain" ? "Replace key" : "Paste API key"} aria-label={`${item.label} API key`} /><button type="submit" disabled={saving}>{item.keySource === "keychain" ? "Replace" : "Connect"}</button></div></form>
        <div className="connection-actions"><button type="button" disabled={saving || item.keySource === "none" || snapshot.settings.modelProvider === item.id} onClick={() => void perform(() => native.selectProvider(item.id))}>{snapshot.settings.modelProvider === item.id ? "In use" : "Use for chat"}</button>{item.keySource === "keychain" && <button type="button" className="remove-key" disabled={saving} onClick={() => void perform(() => native.removeProviderKey(item.id), item.id === "openrouter")}>Remove key</button>}</div>
      </div>}
    </div>)}</div></section>
    <section className="settings-section"><h2>Usage &amp; billing</h2><p className="quiet-note">No Bench account or subscription is needed. Your connected provider bills you for usage.</p><OpenRouterUsage key={`${snapshot.providers.find((item) => item.id === "openrouter")?.keySource}:${connectionRevision}`} connected={snapshot.providers.some((item) => item.id === "openrouter" && item.keySource !== "none")} loadUsage={() => native.openRouterUsage()} onManageCredits={() => native.openOpenRouterBilling()} /></section>
    <section className="settings-section"><h2>Behavior</h2>
      <div className="setting-row"><div className="setting-label">Execution</div><div className="segmented" role="group" aria-label="Execution behavior">{([ ["discuss", "Discuss first"], ["balanced", "Balanced"], ["just_do_it", "Just do it"] ] as const).map(([value, label]) => <button key={value} type="button" aria-pressed={snapshot.settings.executionBehavior === value} onClick={() => onSaveBehaviors(value, snapshot.settings.approvalBehavior)}>{label}</button>)}</div></div>
      <div className="setting-row"><div className="setting-label">Approvals</div><div className="segmented" role="group" aria-label="Approval behavior">{([ ["always", "Always ask"], ["important", "Important actions"], ["autonomous", "Autonomous"] ] as const).map(([value, label]) => <button key={value} type="button" aria-pressed={snapshot.settings.approvalBehavior === value} onClick={() => onSaveBehaviors(snapshot.settings.executionBehavior, value)}>{label}</button>)}</div></div>
    </section>
    <section className="settings-section"><h2>Data &amp; privacy</h2><p className="quiet-note">Conversations and websites are saved on this Mac. API keys are stored in macOS Keychain.</p><p className="quiet-note">Your requests and relevant conversation content are sent to the selected provider. Web search uses OpenRouter when enabled and available.</p></section>
    <section className="settings-section advanced-section"><h2>Advanced</h2>
      <div className="web-search-row"><div><div className="setting-label">Web search</div><div className="advanced-state">{snapshot.webSearchStatus === "openrouter" ? "Available with OpenRouter" : snapshot.webSearchStatus === "off" ? "Off" : "Unavailable"}</div></div><button type="button" className="setting-switch" role="switch" aria-label="Web search" aria-checked={snapshot.settings.webSearchBackend !== "off"} disabled={saving} onClick={() => void perform(() => native.configureWebSearch(snapshot.settings.webSearchBackend === "off" ? "auto" : "off", ""))}><span /></button></div>
      <details><summary>Model IDs <Icon name="chevron" size={16} /></summary><div className="model-list">{snapshot.providers.filter((item) => item.id === "openrouter").map((item) => <form key={item.id} className="model-row" onSubmit={(event) => { event.preventDefault(); void perform(() => native.updateModel(item.id, models[item.id] ?? item.model)); }}><label htmlFor={`model-${item.id}`}>{item.label}</label><input id={`model-${item.id}`} value={models[item.id] ?? item.model} onChange={(event) => setModels((old) => ({ ...old, [item.id]: event.target.value }))} spellCheck={false} /><button type="submit" disabled={saving || (models[item.id] ?? item.model) === item.model}>Save</button></form>)}</div></details>
    </section>
    <section className="settings-section"><h2>About &amp; updates</h2>{version && <p className="quiet-note">Bench {version}</p>}<p className="quiet-note">Updates are installed manually. Automatic updates are not available yet.</p></section>
  </section>;
}
function Sidebar({ snapshot, collapsed, onToggle, expanded, setExpanded, current, selectedId, onNew, onSearch, onSelect, onSettings, onCreateProject }: {
  snapshot: Snapshot | null; collapsed: boolean; onToggle: () => void; expanded: Record<string, boolean>; setExpanded: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
  current?: Conversation; selectedId: string | null; onNew: () => void; onSearch: () => void; onSelect: (id: string) => void; onSettings: () => void; onCreateProject: (name: string) => Promise<void>;
}) {
  const [newProject, setNewProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  return <aside className={`sidebar ${collapsed ? "sidebar-collapsed" : ""}`} aria-label="Sidebar">
    <div className="brand-row" onMouseDown={(event) => { if (event.target === event.currentTarget) beginWindowDrag(event); }}><button type="button" className="brand-mark tool-tip" data-tip={collapsed ? "Expand sidebar" : undefined} onClick={collapsed ? onToggle : undefined} aria-label={collapsed ? "Expand sidebar" : "Bench"} tabIndex={collapsed ? 0 : -1}><Icon name="mark" size={24} /></button><span className="brand-name rail-label" data-tauri-drag-region>Bench</span><button className="icon-button sidebar-collapse" onClick={onToggle} title="Collapse sidebar" aria-label="Collapse sidebar"><Icon name="panel" size={17} /></button></div>
    <div className="sidebar-actions"><button className="action-button tool-tip" data-tip={collapsed ? "New chat" : undefined} onClick={onNew} aria-label="New chat"><Icon name="plus" size={20} /><span className="rail-label">New chat</span></button><button className="action-button tool-tip" data-tip={collapsed ? "Search" : undefined} onClick={onSearch} aria-label="Search"><Icon name="search" size={19} /><span className="rail-label">Search</span></button></div>
    <div className="palette-gap" />
    {!collapsed && <div className="project-create"><button className="icon-button" onClick={() => setNewProject((old) => !old)} aria-label="Create project" title="Create project"><Icon name="plus" size={15} /></button></div>}
    {newProject && !collapsed && <form className="new-project-form" onSubmit={(event) => { event.preventDefault(); void onCreateProject(projectName).then(() => { setProjectName(""); setNewProject(false); }); }}><input autoFocus aria-label="Project name" placeholder="Project name" value={projectName} onChange={(event) => setProjectName(event.target.value)} /><button type="submit" disabled={!projectName.trim()} aria-label="Save project"><Icon name="arrow" size={16} /></button></form>}
    <nav className="project-list" aria-label="Projects">{snapshot?.projects.map((project: Project) => {
      const chats = snapshot.conversations.filter((conversation) => conversation.projectId === project.id);
      const isExpanded = expanded[project.id] ?? false;
      return <div className="project-group" key={project.id}><button className={`project-row tool-tip ${current?.projectId === project.id ? "project-active" : ""}`} data-tip={collapsed ? project.name : undefined} title={collapsed ? project.name : undefined} onClick={() => { if (collapsed) { if (chats[0]) onSelect(chats[0].id); else onToggle(); } else setExpanded((old) => ({ ...old, [project.id]: !isExpanded })); }} aria-label={project.name} aria-expanded={collapsed ? undefined : isExpanded}><span className="project-mark">{project.isSystem ? <Icon name="folder" size={16} /> : project.name.charAt(0).toUpperCase()}</span><span className="truncate rail-label">{project.name}</span><span className={`row-chevron rail-label ${isExpanded ? "expanded" : ""}`}><Icon name="chevron" size={13} /></span></button>{!collapsed && isExpanded && chats.map((conversation) => <button key={conversation.id} className={`chat-row ${selectedId === conversation.id ? "chat-active" : ""}`} onClick={() => onSelect(conversation.id)} title={conversation.title}><span className="truncate">{conversation.title}</span></button>)}</div>;
    })}</nav>
    <div className="sidebar-footer"><button className="profile-button tool-tip" data-tip={collapsed ? "Settings" : undefined} onClick={onSettings} aria-label="Settings"><span className="avatar">B</span><span className="rail-label">Settings</span></button></div>
  </aside>;
}
function OutputWorkspace({ type, state, revisions, busy, onClose, onRestore }: { type: OutputType; state: WebsiteState | null; revisions: WebsiteRevision[]; busy: boolean; onClose: () => void; onRestore: (revision: number) => void }) {
  const Module = workspaceModules[type];
  return <section className="output-workspace" aria-label={`${outputName(type)} workspace`}><div className="output-toolbar"><span>{outputName(type)}</span><button className="icon-button" onClick={onClose} aria-label="Close output workspace"><Icon name="close" size={17} /></button></div>
    {Module && state ? <Module state={state} revisions={revisions} busy={busy} onRestore={onRestore} /> : <div className="output-empty"><span>{busy && type === "website" ? "Building website…" : `${outputName(type)} workspace`}</span>{!busy && type !== "website" && <small>This output is not available yet.</small>}</div>}
  </section>;
}
function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [toolActivity, setToolActivity] = useState<ToolActivity[]>([]);
  const [draft, setDraft] = useState("");
  const [response, setResponse] = useState("");
  const [toolStatus, setToolStatus] = useState("");
  const [outputType, setOutputType] = useState<OutputType>("auto");
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({ miscellaneous: true });
  const [view, setView] = useState<"workspace" | "search" | "settings" | "setup">("workspace");
  const connectionAttempt = useRef(0);
  const [setupReturn, setSetupReturn] = useState<"workspace" | "settings">("workspace");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Conversation[]>([]);
  const [error, setError] = useState("");
  const [failedId, setFailedId] = useState<string | null>(null);
  const [failedReason, setFailedReason] = useState("");
  const [website, setWebsite] = useState<WebsiteState | null>(null);
  const [revisions, setRevisions] = useState<WebsiteRevision[]>([]);
  const [pendingReview, setPendingReview] = useState<PendingReview | null>(null);
  const selectedRef = useRef<string | null>(null);
  const [outputOpen, setOutputOpen] = useState(true);
  const [projectChanging, setProjectChanging] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);
  const lastProject = useRef<{ id: string; projectId: string } | null>(null);
  const gridPointer = useRef({ x: 0, y: 0 });
  const gridFrame = useRef<number | null>(null);
  useEffect(() => () => { if (gridFrame.current !== null) window.cancelAnimationFrame(gridFrame.current); }, []);
  const moveGrid = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === "touch") return;
    gridPointer.current = { x: event.clientX, y: event.clientY };
    if (gridFrame.current !== null) return;
    const shell = event.currentTarget;
    gridFrame.current = window.requestAnimationFrame(() => {
      gridFrame.current = null;
      const bounds = shell.getBoundingClientRect();
      shell.style.setProperty("--grid-x", `${gridPointer.current.x - bounds.left}px`);
      shell.style.setProperty("--grid-y", `${gridPointer.current.y - bounds.top}px`);
      shell.style.setProperty("--grid-active", "1");
    });
  };
  const leaveGrid = (event: React.PointerEvent<HTMLDivElement>) => {
    if (gridFrame.current !== null) window.cancelAnimationFrame(gridFrame.current);
    gridFrame.current = null;
    event.currentTarget.style.setProperty("--grid-active", "0");
  };
  const refresh = async () => { const next = await native.snapshot(); setSnapshot(next); return next; };
  useEffect(() => { native.snapshot().then((next) => { setSnapshot(next); setCollapsed(next.settings.sidebarCollapsed); if (next.conversations.length === 0 && next.providers.length > 0 && next.providers.every((provider) => provider.keySource === "none")) setView("setup"); }).catch((e) => setError(errorText(e))); }, []);
  useEffect(() => { endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" }); }, [messages, response, pendingReview]);
  useEffect(() => { if (view !== "search" || !searchQuery.trim()) { setSearchResults([]); return; } let live = true; const timer = window.setTimeout(() => { native.search(searchQuery).then((results) => { if (live) setSearchResults(results); }).catch((e) => setError(errorText(e))); }, 180); return () => { live = false; window.clearTimeout(timer); }; }, [searchQuery, view]);
  const selectConversation = async (id: string) => {
    selectedRef.current = id; setSelectedId(id); setPendingReview(null); setResponse(""); setToolStatus(""); setFailedId(null); setError(""); setMessages([]); setToolActivity([]); setWebsite(null); setRevisions([]); setView("workspace"); setOutputOpen(true);
    try {
      const [loaded, site, activity] = await Promise.all([native.messages(id), native.loadWebsite(id), native.toolActivity(id)]);
      if (selectedRef.current !== id) return;
      setMessages(loaded); setWebsite(site); setToolActivity(activity);
      const conversation = snapshot?.conversations.find((item) => item.id === id);
      setOutputType(conversation?.outputSelection ?? "auto");
      const pendingChat = conversation?.outputType === "chat" && loaded[loaded.length - 1]?.role === "user";
      const pendingWebsite = conversation?.outputType === "website" && loaded.filter((message) => message.role === "user").length > (site?.requestCount ?? 0);
      if (pendingChat || pendingWebsite) { setFailedId(id); setFailedReason("Response not completed."); }
      if (site) { const history = await native.websiteRevisions(id); if (selectedRef.current === id) setRevisions(history); }
    } catch (e) { if (selectedRef.current === id) setError(errorText(e)); }
  };
  const startNew = () => { selectedRef.current = null; setPendingReview(null); setRevisions([]); setSelectedId(null); setMessages([]); setToolActivity([]); setResponse(""); setToolStatus(""); setDraft(""); setOutputType("auto"); setWebsite(null); setView("workspace"); setError(""); setFailedId(null); };
  const executeWebsite = async (conversationId: string, operation: "generate" | "restore", targetRevision?: number, token?: string) => {
    const site = operation === "restore" && targetRevision !== undefined
      ? await native.restoreWebsite(conversationId, targetRevision, token)
      : await native.generateWebsite(conversationId, token);
    if (selectedRef.current === conversationId) setWebsite(site);
    try { const history = await native.websiteRevisions(conversationId); if (selectedRef.current === conversationId) setRevisions(history); }
    catch { if (selectedRef.current === conversationId) { setRevisions([{ revision: site.revision, current: true }]); setError("Website saved. Version history is temporarily unavailable."); } }
  };
  const requestWebsiteAction = async (conversationId: string, operation: "generate" | "restore", targetRevision?: number) => {
    const review = await native.prepareWebsiteAction(conversationId, operation, targetRevision);
    if (review) {
      if (selectedRef.current === conversationId) setPendingReview({ review, operation, targetRevision });
      return;
    }
    await executeWebsite(conversationId, operation, targetRevision);
  };
  const runOutput = async (conversation: Conversation) => {
    if (conversation.outputType === "website") { if (selectedRef.current === conversation.id) setOutputOpen(true); await requestWebsiteAction(conversation.id, "generate"); }
    else if (conversation.outputType === "chat") await native.stream(conversation.id, (event) => { if (selectedRef.current !== conversation.id) return; if (event.kind === "delta") setResponse((old) => old + event.text); if (event.kind === "tool") setToolStatus(event.text); });
  };
  const approvePendingAction = async () => {
    if (!pendingReview || busy) return;
    const pending = pendingReview;
    setBusy(true); setBusyId(pending.review.conversationId); setPendingReview(null); if (pending.operation === "generate") setFailedId(null);
    try {
      const token = await native.approveAction(pending.review.id);
      await executeWebsite(pending.review.conversationId, pending.operation, pending.targetRevision, token);
      await refresh();
    } catch (e) {
      if (selectedRef.current === pending.review.conversationId) {
        if (pending.operation === "generate") { setFailedId(pending.review.conversationId); setFailedReason(responseError(e)); }
        else setError("Couldn’t restore this version. Your current website is still here.");
      }
    } finally { setBusy(false); setBusyId(null); }
  };
  const restoreWebsite = async (revision: number) => {
    if (!selectedId || busy) return;
    setBusy(true); setBusyId(selectedId); setPendingReview(null); setError("");
    try { await requestWebsiteAction(selectedId, "restore", revision); }
    catch { setError("Couldn’t restore this version. Your current website is still here."); }
    finally { setBusy(false); setBusyId(null); }
  };
  const ensureConnection = (saved = false) => {
    if (!snapshot) { setError("Bench is still loading. Your draft is here; try again shortly."); return false; }
    if (!snapshot.providers.some((item) => item.id === snapshot.settings.modelProvider && item.keySource !== "none")) { if (snapshot.settings.modelProvider === "openrouter") { setSetupReturn("workspace"); setView("setup"); } else setView("settings"); setError(saved ? "Connect a provider to continue. Your request is saved." : "Connect a provider to continue. Your draft is still here."); return false; }
    return true;
  };
  const send = async () => {
    if (!draft.trim() || busy) return;
    if (!snapshot) { ensureConnection(); return; }
    const origin = selectedRef.current;
    const request = draft.trim();
    setBusy(true); setBusyId(origin); setPendingReview(null); setError(""); setResponse(""); setToolStatus(""); setFailedId(null);
    let created: Conversation | null = null;
    try {
      created = await native.createMessage(origin, request, outputType);
      setBusyId(created.id);
      if (selectedRef.current === origin) {
        setDraft(""); selectedRef.current = created.id; setSelectedId(created.id); setOutputOpen(true);
        setExpanded((old) => ({ ...old, [created!.projectId]: true }));
        const loaded = await native.messages(created.id);
        if (selectedRef.current === created.id) setMessages(loaded);
      }
      await refresh();
      if ((created.outputType === "chat" || created.outputType === "website") && !ensureConnection(true)) {
        if (selectedRef.current === created.id) { setFailedId(created.id); setFailedReason("Request saved. Connect a provider in Settings to continue."); }
        return;
      }
      await runOutput(created);
      const loaded = await native.messages(created.id);
      if (selectedRef.current === created.id) { setMessages(loaded); setToolActivity(await native.toolActivity(created.id)); setToolStatus(""); setResponse(""); }
      await refresh();
    } catch (e) {
      if (created) { if (selectedRef.current === created.id) { setFailedId(created.id); setFailedReason(responseError(e)); setResponse(""); } }
      else if (selectedRef.current === origin) setError("Couldn’t save your message. Your draft is still here.");
    } finally { setBusy(false); setBusyId(null); }
  };
  const retry = async () => {
    if (!failedId || busy || !snapshot) return;
    const conversation = snapshot.conversations.find((item) => item.id === failedId); if (!conversation) return;
    if (!ensureConnection(true)) return;
    setBusy(true); setBusyId(conversation.id); setPendingReview(null); setResponse(""); setToolStatus(""); setFailedId(null);
    try { await runOutput(conversation); if (selectedRef.current === conversation.id) { setMessages(await native.messages(conversation.id)); setToolActivity(await native.toolActivity(conversation.id)); setToolStatus(""); setResponse(""); } await refresh(); } catch (e) { if (selectedRef.current === conversation.id) { setToolActivity(await native.toolActivity(conversation.id).catch(() => [])); setToolStatus(""); setFailedId(conversation.id); setFailedReason(responseError(e)); setResponse(""); } } finally { setBusy(false); setBusyId(null); }
  };
  const saveSettings = async (execution: ExecutionBehavior, approval: ApprovalBehavior) => { try { await native.settings(execution, approval); await refresh(); setError(""); } catch (e) { setError(errorText(e)); } };
  const toggleSidebar = () => { const next = !collapsed; setCollapsed(next); native.setSidebarCollapsed(next).then((settings) => setSnapshot((old) => old ? { ...old, settings } : old)).catch((e) => { setCollapsed(!next); setError(errorText(e)); }); };
  const addProject = async (name: string) => { try { await native.createProject(name); await refresh(); setError(""); } catch (e) { setError(errorText(e)); throw e; } };
  const moveCurrent = async (projectId: string) => { if (!selectedId) return; try { await native.move(selectedId, projectId); setExpanded((old) => ({ ...old, [projectId]: true })); setProjectChanging(true); window.setTimeout(() => setProjectChanging(false), 320); await refresh(); setError(""); } catch (e) { setError(errorText(e)); } };
  const current = snapshot?.conversations.find((conversation) => conversation.id === selectedId);
  useEffect(() => { if (current && lastProject.current?.id === current.id && lastProject.current.projectId !== current.projectId) { setProjectChanging(true); const timer = window.setTimeout(() => setProjectChanging(false), 320); lastProject.current = { id: current.id, projectId: current.projectId }; return () => window.clearTimeout(timer); } lastProject.current = current ? { id: current.id, projectId: current.projectId } : null; }, [current?.id, current?.projectId]);
  const activeBusy = busy && busyId === selectedId;
  const hasOutput = !!current && current.outputType !== "chat" && current.outputType !== "auto";
  return <div className="app-shell" onPointerMove={moveGrid} onPointerLeave={leaveGrid}><div className="drag-strip" data-tauri-drag-region aria-hidden="true" />
    <Sidebar snapshot={snapshot} collapsed={collapsed} onToggle={toggleSidebar} expanded={expanded} setExpanded={setExpanded} current={current} selectedId={selectedId} onNew={startNew} onSearch={() => { setView("search"); setSearchQuery(""); }} onSelect={(id) => void selectConversation(id)} onSettings={() => setView("settings")} onCreateProject={addProject} />
    <main className={`main-area ${collapsed ? "with-rail" : "with-sidebar"}`}>
      {view === "workspace" && !selectedId && <div className="empty-workspace"><Composer value={draft} onChange={setDraft} onSend={send} busy={busy} outputType={outputType} setOutputType={setOutputType} centered definitions={snapshot?.outputs ?? []} /></div>}
      {view === "workspace" && selectedId && <div className={`adaptive-workspace ${hasOutput && outputOpen ? "has-output" : ""}`}>
        <div className="conversation-view"><div className="message-scroll"><div className={`message-stack ${projectChanging ? "project-changing" : ""}`}>
          <header className="conversation-header"><span className="conversation-title">{current?.title}</span><label className="project-picker"><span className="sr-only">Project</span><select value={current?.projectId ?? "miscellaneous"} onChange={(event) => void moveCurrent(event.target.value)} aria-label="Move conversation to project">{snapshot?.projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select><Icon name="chevron" size={13} /></label>{hasOutput && !outputOpen && <button className="show-output" onClick={() => setOutputOpen(true)}>Show {outputName(current.outputType)}</button>}</header>
          {messages.map((message) => <div key={message.id} className={`message message-${message.role}`}><div className="message-content">{message.role === "assistant" ? <Markdown text={message.content} /> : message.content}</div>{message.role === "user" && toolActivity.filter((activity) => activity.userMessageId === message.id).map((activity) => <div className="tool-activity" key={activity.id}>{activity.status === "completed" ? activity.summary : "Web search unavailable"}</div>)}</div>)}
          {toolActivity.filter((activity) => activity.userMessageId === null).map((activity) => <div className="tool-activity" key={activity.id}>{activity.status === "completed" ? activity.summary : "Web search unavailable"}</div>)}
          {toolStatus && <div className="tool-status">{toolStatus}</div>}
          {response && <div className="message message-assistant"><div className="message-content"><Markdown text={response} /></div></div>}
          {activeBusy && !response && <div className="thinking" aria-label="Waiting for response"><span /><span /><span /></div>}
          {pendingReview?.review.conversationId === selectedId && <div className="action-review"><div><span>{pendingReview.review.title}</span><p>{pendingReview.review.detail}</p></div><div className="action-review-controls"><button type="button" disabled={busy} onClick={() => void approvePendingAction()}>{pendingReview.operation === "restore" ? "Restore" : "Continue"}</button><button type="button" disabled={busy} onClick={() => { setPendingReview(null); if (pendingReview.operation === "generate") { setFailedId(selectedId); setFailedReason("Request saved. Continue when ready."); } }}>Not now</button></div></div>}
          {failedId === selectedId && <div className="inline-failure"><span>{failedReason}</span><button onClick={() => void retry()}>Retry</button></div>}<div ref={endRef} />
        </div></div><div className="conversation-composer"><Composer value={draft} onChange={setDraft} onSend={send} busy={busy} outputType={outputType} setOutputType={setOutputType} centered={false} definitions={snapshot?.outputs ?? []} /></div></div>
        {hasOutput && outputOpen && <OutputWorkspace type={current.outputType} state={website} revisions={revisions} busy={activeBusy} onClose={() => setOutputOpen(false)} onRestore={(revision) => void restoreWebsite(revision)} />}
      </div>}
      {view === "search" && <section className="utility-panel search-panel"><div className="panel-top"><h1>Search</h1><button className="icon-button" onClick={() => setView("workspace")} aria-label="Close search"><Icon name="close" /></button></div><div className="search-input-wrap"><Icon name="search" size={19} /><input autoFocus placeholder="Search conversations" value={searchQuery} onChange={(event) => setSearchQuery(event.target.value)} aria-label="Search conversations" /></div><div className="search-results">{searchResults.map((conversation) => <button key={conversation.id} className="search-result" onClick={() => void selectConversation(conversation.id)}><span>{conversation.title}</span><small>{snapshot?.projects.find((project) => project.id === conversation.projectId)?.name ?? "Miscellaneous"}</small></button>)}{searchQuery && searchResults.length === 0 && <p className="quiet-note">No conversations found.</p>}</div></section>}
      {view === "setup" && <OpenRouterSetup onCancelConnect={() => { connectionAttempt.current++; return native.cancelOpenRouterConnect(); }} onConnect={async () => { const attempt = ++connectionAttempt.current; await native.connectOpenRouter(); if (attempt !== connectionAttempt.current) return; const next = await refresh(); if (attempt !== connectionAttempt.current) return; if (!next.providers.some((provider) => provider.id === "openrouter" && provider.keySource !== "none")) throw new Error("Connection not saved"); setView(setupReturn); setError(""); }} onDismiss={() => { setView(setupReturn); setError(""); }} />}
      {view === "settings" && snapshot && <SettingsView snapshot={snapshot} onClose={() => setView("workspace")} onSaveBehaviors={(execution, approval) => void saveSettings(execution, approval)} onRefresh={async () => { await refresh(); }} onError={setError} onSetupOpenRouter={() => { setSetupReturn("settings"); setView("setup"); setError(""); }} />}
      {error && <div className="error-toast" role="alert"><span>{error}</span><button onClick={() => setError("")} aria-label="Dismiss error"><Icon name="close" size={15} /></button></div>}
    </main>
  </div>;
}
export default App;
