import { useEffect, useRef, useState } from "react";

type OpenRouterSetupProps = {
  onConnect: () => Promise<void>;
  onCancelConnect: () => Promise<void>;
  onDismiss: () => void;
};

export function OpenRouterSetup({ onConnect, onCancelConnect, onDismiss }: OpenRouterSetupProps) {
  const working = useRef(false);
  const latestCancel = useRef(onCancelConnect);
  latestCancel.current = onCancelConnect;
  const mounted = useRef(true);
  const attempt = useRef(0);
  const [busy, setBusy] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const cancelWorking = useRef(false);
  const [error, setError] = useState("");
  const [connected, setConnected] = useState(false);
  useEffect(() => { mounted.current = true; return () => {
    mounted.current = false; attempt.current++;
    if (working.current && !cancelWorking.current) {
      working.current = false;
      try { void Promise.resolve(latestCancel.current()).catch(() => {}); } catch {}
    }
  }; }, []);
  const connect = async () => {
    if (working.current || connected) return;
    working.current = true; const id = ++attempt.current;
    setBusy(true); setError("");
    const current = () => mounted.current && attempt.current === id;
    try { await onConnect(); if (current()) setConnected(true); }
    catch { if (current()) setError("Couldn’t connect to OpenRouter. Try again."); }
    finally { if (current()) { working.current = false; setBusy(false); } }
  };
  const cancel = async () => {
    if (!working.current || cancelWorking.current) return;
    cancelWorking.current = true; setCancelling(true); setError("");
    // Invalidate browser completion before invoking native cancellation.
    const id = ++attempt.current;
    try {
      await onCancelConnect();
      if (mounted.current && attempt.current === id) { working.current = false; setBusy(false); }
    } catch {
      if (mounted.current && attempt.current === id) setError("Couldn’t cancel the connection. Try cancelling again.");
    } finally { if (mounted.current && attempt.current === id) { cancelWorking.current = false; setCancelling(false); } }
  };
  return <section className="utility-panel openrouter-setup" aria-labelledby="openrouter-setup-title">
    <div className="panel-top"><h1 id="openrouter-setup-title">Connect OpenRouter</h1></div>
    <p className="setup-intro">One connection gives Bench access to AI models for Chat and Website.</p>
    <p className="setup-intro">Sign in or create an account in your browser. You’ll return to Bench automatically when connected.</p>
    <div className="field-row"><button type="button" disabled={busy || connected} onClick={() => void connect()}>{connected ? "Connected" : busy ? "Waiting for OpenRouter…" : "Connect OpenRouter"}</button>{busy && <button type="button" disabled={cancelling} onClick={() => void cancel()}>{cancelling ? "Cancelling…" : "Cancel connection"}</button>}</div>
    <p className="quiet-note">OpenRouter bills you directly. Add credits in OpenRouter if your account needs them. Your connection credential stays in macOS Keychain. Connecting does not make a model request.</p>
    <p className="quiet-note">Your requests and relevant conversation content are sent to OpenRouter when you use Bench.</p>
    {error && <p className="setup-error" role="alert">{error}</p>}
    {connected && <p role="status" className="quiet-note">OpenRouter is connected. You can continue in Bench.</p>}
    <div className="connection-actions"><button type="button" disabled={busy} onClick={onDismiss}>{connected ? "Continue" : "Set up later"}</button></div>
  </section>;
}
