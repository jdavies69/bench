import { useEffect, useRef, useState } from "react";

export type OpenRouterUsageData = {
  usageDaily: number; usageWeekly: number; usageMonthly: number; usageTotal: number;
  limit: number | null; limitRemaining: number | null; limitReset: string | null;
  byokUsageMonthly: number | null;
};
type Props = {
  connected: boolean;
  loadUsage: () => Promise<OpenRouterUsageData>;
  onManageCredits: () => Promise<void>;
  connectionVersion?: string | number;
};
const usd = (amount: number) => new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 4 }).format(amount);
const valid = (data: OpenRouterUsageData) => data && [data.usageDaily, data.usageWeekly, data.usageMonthly, data.usageTotal].every((v) => Number.isFinite(v) && v >= 0) && [data.limit, data.limitRemaining, data.byokUsageMonthly].every((v) => v === null || Number.isFinite(v) && v >= 0);

export function OpenRouterUsage({ connected, loadUsage, onManageCredits, connectionVersion }: Props) {
  const [data, setData] = useState<OpenRouterUsageData | null>(null);
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const [busy, setBusy] = useState<"usage" | "manage" | null>(null);
  const [error, setError] = useState("");
  const [stale, setStale] = useState(false);
  const epoch = useRef(0);
  const working = useRef(false);
  const live = useRef(true);
  useEffect(() => {
    live.current = true; epoch.current++; working.current = false;
    setData(null); setCheckedAt(null); setBusy(null); setError(""); setStale(false);
    return () => { live.current = false; epoch.current++; working.current = false; };
  }, [connected, connectionVersion]);
  const perform = async (operation: "usage" | "manage") => {
    if (!connected || working.current) return;
    working.current = true; const version = epoch.current;
    setBusy(operation); setError("");
    const current = () => live.current && epoch.current === version;
    try {
      if (operation === "usage") {
        const result = await loadUsage();
        if (!valid(result)) throw new Error("Invalid usage response");
        if (current()) { setData(result); setCheckedAt(new Date()); setStale(false); }
      } else await onManageCredits();
    } catch {
      if (current()) {
        setError(operation === "usage" ? "Couldn’t check usage. Try again." : "Couldn’t open OpenRouter. Try again.");
        if (operation === "usage") setStale(true);
      }
    } finally { if (current()) { working.current = false; setBusy(null); } }
  };
  if (!connected) return <p className="quiet-note">Connect OpenRouter to see usage.</p>;
  const metrics = data ? [
    ["Today (UTC)", usd(data.usageDaily)],
    ["This month (UTC)", usd(data.usageMonthly)],
    ["Lifetime", usd(data.usageTotal)],
    ["Key spending limit", data.limit === null ? "Unlimited" : usd(data.limit)],
    ["Key limit remaining", data.limit === null ? "Not applicable" : data.limitRemaining === null ? "Unavailable" : usd(data.limitRemaining)],
    ...(data.byokUsageMonthly === null ? [] : [["BYOK this month (UTC)", usd(data.byokUsageMonthly)]]),
  ] : [];
  return <section className="usage-details" aria-label="OpenRouter usage">
    <div className="connection-actions"><button type="button" disabled={busy !== null} onClick={() => void perform("usage")}>{busy === "usage" ? "Checking…" : data ? "Refresh usage" : "Check usage"}</button><button type="button" disabled={busy !== null} onClick={() => void perform("manage")}>Manage credits</button></div>
    {data && <dl className="usage-metrics">{metrics.map(([label, value]) => <div className="usage-row" key={label}><dt className="metric-label">{label}</dt><dd className="metric-value">{value}</dd></div>)}</dl>}
    {checkedAt && <p className="quiet-note">Last successful check: <time dateTime={checkedAt.toISOString()}>{checkedAt.toLocaleString()}</time>{stale ? ". These figures may be out of date." : ""}</p>}
    {error && <p className="usage-error" role="alert">{error}</p>}
    <p className="quiet-note">USD usage across all apps using this API key. The key’s spending limit is separate from your OpenRouter account credit balance.</p>
  </section>;
}
