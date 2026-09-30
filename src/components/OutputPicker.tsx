import { useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { OutputDefinition, OutputType } from "../native";

function OutputIcon({ type }: { type: OutputType }) {
  const paths: Record<OutputType, React.ReactNode> = {
    auto: <><path d="M4 7h16M4 17h16" /><circle cx="9" cy="7" r="2" fill="white" /><circle cx="15" cy="17" r="2" fill="white" /></>,
    chat: <path d="M5 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H9l-6 3V6a2 2 0 0 1 2-2Z" />,
    website: <><circle cx="12" cy="12" r="9" /><ellipse cx="12" cy="12" rx="4" ry="9" /><path d="M3 12h18" /></>,
    application: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></>,
    presentation: <><rect x="3" y="4" width="18" height="13" rx="1" /><path d="M12 17v4M8 21h8" /></>,
    document: <><path d="M6 3h8l4 4v14H6Z" /><path d="M14 3v5h4" /></>,
    image: <><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="8" cy="8" r="1" /><path d="m3 17 6-6 4 4 3-3 5 5" /></>,
    agent: <><circle cx="12" cy="7" r="4" /><path d="M4 21v-2a8 8 0 0 1 16 0v2Z" /></>,
    voice: <path d="M4 10v4M8 6v12M12 3v18M16 6v12M20 10v4" />,
  };
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[type]}</svg>;
}

export function OutputPicker({ value, onChange, definitions }: {
  value: OutputType; onChange: (value: OutputType) => void; definitions: OutputDefinition[];
}) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ left: 0, top: 0, maxHeight: 0 });
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const options = [
    { id: "auto" as const, label: "Auto", implemented: true },
    ...definitions.filter((item) => item.implemented),
    ...definitions.filter((item) => !item.implemented),
  ];
  const label = options.find((item) => item.id === value)?.label ?? "Auto";
  const close = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  };
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      if (!trigger.current || !menu.current) return;
      const anchor = trigger.current.getBoundingClientRect();
      const above = Math.max(0, anchor.top - 20);
      const below = Math.max(0, window.innerHeight - anchor.bottom - 20);
      const naturalHeight = menu.current.scrollHeight + 2;
      const upwards = above >= naturalHeight || above >= below;
      const maxHeight = upwards ? above : below;
      const height = Math.min(naturalHeight, maxHeight);
      setPosition({ left: Math.max(12, Math.min(anchor.right - 232, window.innerWidth - 244)), top: upwards ? anchor.top - height - 8 : anchor.bottom + 8, maxHeight });
    };
    place();
    const selected = menu.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]');
    selected?.focus({ preventScroll: true });
    selected?.scrollIntoView({ block: "nearest" });
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !menu.current?.contains(event.target) && !trigger.current?.contains(event.target)) setOpen(false);
    };
    const blur = (event: FocusEvent) => {
      if (event.target instanceof Node && !menu.current?.contains(event.target) && !trigger.current?.contains(event.target)) setOpen(false);
    };
    window.addEventListener("resize", place);
    document.addEventListener("pointerdown", outside);
    document.addEventListener("focusin", blur);
    return () => {
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("focusin", blur);
    };
  }, [open, definitions]);

  return <>
    <button ref={trigger} type="button" className="output-picker" aria-label={`Output type: ${label}`} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined}
      onClick={() => setOpen(!open)} onKeyDown={(event) => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setOpen(true); }
        if (event.key === "Escape") close();
      }}><span>{label}</span><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" aria-hidden="true"><path d="m7 10 5 5 5-5" /></svg></button>
    {open && createPortal(<div ref={menu} id={id} role="menu" aria-label="Output type" className="output-menu" style={position} onKeyDown={(event) => {
      const items = [...(menu.current?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]') ?? [])];
      const current = items.indexOf(document.activeElement as HTMLButtonElement);
      let next: number | undefined;
      if (event.key === "ArrowDown") next = (current + 1) % items.length;
      if (event.key === "ArrowUp") next = (current - 1 + items.length) % items.length;
      if (event.key === "Home") next = 0;
      if (event.key === "End") next = items.length - 1;
      if (next !== undefined) { event.preventDefault(); items[next]?.focus(); }
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(true); }
      if (event.key === "Tab") { close(true); } // Let normal tab order continue from the trigger.
      if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && event.key !== " ") {
        const rotated = [...items.slice(current + 1), ...items.slice(0, current + 1)];
        rotated.find((item) => item.textContent?.trim().toLowerCase().startsWith(event.key.toLowerCase()))?.focus();
      }
    }}>{options.map((item, index) => <div key={item.id}>
      {!item.implemented && options[index - 1]?.implemented && <div className="output-menu-later">Coming later</div>}
      <button type="button" role="menuitemradio" aria-checked={item.id === value} aria-describedby={!item.implemented ? `${id}-later` : undefined} tabIndex={-1} className={`output-menu-item${!item.implemented ? " output-menu-deferred" : ""}`}
        onClick={() => { onChange(item.id); close(true); }}><OutputIcon type={item.id} /><span>{item.label}</span>{item.id === value && <svg className="output-menu-check" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="m5 12 4 4L19 6" /></svg>}</button>
    </div>)}<span id={`${id}-later`} className="sr-only">Coming later. Requests are saved in a placeholder workspace.</span></div>, document.body)}
  </>;
}
