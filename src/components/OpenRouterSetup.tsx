import { useEffect, useRef, useState } from "react";

type OpenRouterSetupProps = {
  onConnect: (key: string) => Promise<void>;
  onOpenKeys: () => Promise<void>;
  onDismiss: () => void;
  onOtherProviders: () => void;
};

export function OpenRouterSetup({ onConnect, onOpenKeys, onDismiss, onOtherProviders }: OpenRouterSetupProps) {
  const keyInput = useRef<HTMLInputElement>(null);
  const working = useRef(false);
  const mounted = useRef(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [connected, setConnected] = useState(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; if (keyInput.current) keyInput.current.value = ""; }; }, []);
  const connect = async () => {
    if (working.current || connected) return;
    const key = keyInput.current?.value.trim() ?? "";
    if (!key) { setError("Paste your OpenRouter API key to continue."); keyInput.current?.focus(); return; }
    working.current = true; setBusy(true); setError("");
    try {
      await onConnect(key);
      if (keyInput.current) keyInput.current.value = "";
      if (mounted.current) setConnected(true);
    } catch {
      // Native failures may contain submitted credentials. Never render them.
      if (mounted.current) setError("Couldn’t save your connection. Your key is still here; try again.");
    } finally { working.current = false; if (mounted.current) setBusy(false); }
  };
  const openKeys = async () => {
    if (working.current) return;
    working.current = true; setBusy(true); setError("");
    try { await onOpenKeys(); }
    catch { if (mounted.current) setError("Couldn’t open OpenRouter. Try again."); }
    finally { working.current = false; if (mounted.current) setBusy(false); }
  };
  return <section className="utility-panel openrouter-setup" aria-labelledby="openrouter-setup-title">
    <div className="panel-top"><h1 id="openrouter-setup-title">Connect OpenRouter</h1></div>
    <p className="setup-intro">One connection gives Bench access to AI models for Chat and Website.</p>
    <ol className="setup-steps">
      <li>Sign in to OpenRouter. Add credits if your account needs them.</li>
      <li>Create and copy an API key. You can set a spending limit.</li>
      <li>Return to Bench and paste your key below.</li>
    </ol>
    <button type="button" className="setup-open-keys" disabled={busy} onClick={() => void openKeys()}>Open OpenRouter ↗</button>
    <div className="connection-detail"><form onSubmit={(event) => { event.preventDefault(); void connect(); }}>
      <label htmlFor="setup-openrouter-key">Already have a key? Paste it here.</label>
      <div className="field-row"><input ref={keyInput} autoFocus id="setup-openrouter-key" type="password" autoComplete="off" autoCapitalize="none" spellCheck={false} maxLength={4096} disabled={busy || connected} placeholder="OpenRouter API key" aria-label="OpenRouter API key" aria-describedby="setup-privacy" /><button type="submit" disabled={busy || connected}>{connected ? "Connected" : busy ? "Connecting…" : "Connect"}</button></div>
    </form></div>
    <p id="setup-privacy" className="quiet-note">Your key stays in macOS Keychain. Your requests and relevant conversation content are sent to OpenRouter. OpenRouter bills you directly; saving this connection does not make a model request.</p>
    {error && <p className="setup-error" role="alert">{error}</p>}
    {connected && <p role="status" className="quiet-note">OpenRouter is connected. You can continue in Bench.</p>}
    <div className="connection-actions"><button type="button" disabled={busy} onClick={onDismiss}>{connected ? "Continue" : "Set up later"}</button><button type="button" disabled={busy} onClick={onOtherProviders}>Use another provider</button></div>
  </section>;
}
