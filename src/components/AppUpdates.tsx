import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type UpdateStatus = { status: string; version: string | null; automatic: boolean; message: string };
export function AppUpdates({ blocked = false }: { blocked?: boolean }) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const working = useRef(false);
  const live = useRef(true);
  const requestSequence = useRef(0);
  const refresh = async () => {
    const sequence = ++requestSequence.current;
    try { const value = await invoke<UpdateStatus>("app_update_status"); if (live.current && sequence === requestSequence.current) { setStatus(value); setError(""); } }
    catch { if (live.current && sequence === requestSequence.current) setError("Couldn’t load update information. Try again."); }
  };
  useEffect(() => {
    live.current = true; void refresh();
    const timer = window.setInterval(() => void refresh(), 2000);
    return () => { live.current = false; requestSequence.current++; window.clearInterval(timer); };
  }, []);
  const perform = async (operation: "check" | "install" | "toggle") => {
    if (working.current || operation === "install" && blocked) return;
    working.current = true; setBusy(true); setError("");
    try {
      if (operation === "toggle") await invoke("set_automatic_updates", { enabled: !status?.automatic });
      else if (operation === "install") await invoke("install_app_update");
      else { const sequence = ++requestSequence.current; const value = await invoke<UpdateStatus>("check_app_update"); if (live.current && sequence === requestSequence.current) setStatus(value); }
      if (operation === "toggle") await refresh();
    } catch { if (live.current) setError(operation === "install" ? "Couldn’t install the update. Your saved work is still here." : operation === "check" ? "Couldn’t check for updates. Try again." : "Couldn’t update this setting. Try again."); }
    finally { working.current = false; if (live.current) setBusy(false); }
  };
  const configured = status && status.status !== "unconfigured";
  const checking = status?.status === "checking" || status?.status === "downloading";
  return <div className="app-updates">
    <div className="setting-row"><div className="setting-label">Automatic updates</div><button type="button" role="switch" aria-label="Automatic updates" aria-checked={status?.automatic ?? false} className="setting-switch" disabled={busy || !configured} onClick={() => void perform("toggle")}><span /></button></div>
    <p className="quiet-note">{status?.message || "Checking update information…"}</p>
    {!status && error && <div className="connection-actions"><button type="button" onClick={() => void refresh()}>Retry update information</button></div>}
    {configured && <div className="connection-actions"><button type="button" disabled={busy || checking} onClick={() => void perform("check")}>{checking ? "Checking…" : "Check for updates"}</button>{status.status === "ready" && <button type="button" disabled={busy || blocked} onClick={() => void perform("install")}>Restart to update{status.version ? ` to ${status.version}` : ""}</button>}</div>}
    {status?.status === "ready" && blocked && <p className="quiet-note">Finish the current request and save or clear your draft before restarting.</p>}
    {configured && <p className="quiet-note">Updates download automatically when enabled. Restart when ready to install; your conversations and connections stay on this Mac.</p>}
    {error && <p role="alert" className="usage-error">{error}</p>}
  </div>;
}
