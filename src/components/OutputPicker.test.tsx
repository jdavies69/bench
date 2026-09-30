// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { OutputDefinition, OutputType } from "../native";
import { OutputPicker } from "./OutputPicker";

const definitions: OutputDefinition[] = [
  { id: "chat", label: "Chat", implemented: true, workspace: "conversation" },
  { id: "website", label: "Website", implemented: true, workspace: "canvas" },
  { id: "image", label: "Image", implemented: false, workspace: "canvas" },
];
function Picker() {
  const [value, setValue] = useState<OutputType>("auto");
  return <><OutputPicker value={value} onChange={setValue} definitions={definitions} /><button>Send</button></>;
}
beforeEach(() => { Element.prototype.scrollIntoView = vi.fn(); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("compact output picker", () => {
  it("keeps the menu within the window and repositions on resize", async () => {
    vi.stubGlobal("innerHeight", 760);
    vi.stubGlobal("innerWidth", 680);
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({ top: 356, bottom: 395, left: 525, right: 600, width: 75, height: 39, x: 525, y: 356, toJSON: () => ({}) });
    vi.spyOn(Element.prototype, "scrollHeight", "get").mockReturnValue(341);
    const user = userEvent.setup(); render(<Picker />);
    await user.click(screen.getByRole("button", { name: "Output type: Auto" }));
    const menu = screen.getByRole("menu");
    expect(Number.parseFloat(menu.style.top)).toBeGreaterThan(395);
    expect(Number.parseFloat(menu.style.top) + 343).toBeLessThan(760);
    expect(Number.parseFloat(menu.style.left) + 232).toBeLessThan(680);
    vi.stubGlobal("innerHeight", 500);
    fireEvent(window, new Event("resize"));
    expect(Number.parseFloat(menu.style.top)).toBeGreaterThanOrEqual(12);
    expect(Number.parseFloat(menu.style.top) + Number.parseFloat(menu.style.maxHeight)).toBeLessThan(356);
  });

  it("selects an output without submitting, marks it selected, and restores focus", async () => {
    const submit = vi.fn((event: React.FormEvent) => event.preventDefault());
    const user = userEvent.setup();
    render(<form onSubmit={submit}><Picker /></form>);
    await user.click(screen.getByRole("button", { name: "Output type: Auto" }));
    expect(screen.getByRole("menuitemradio", { name: "Auto" }).getAttribute("aria-checked")).toBe("true");
    await user.click(screen.getByRole("menuitemradio", { name: "Website" }));
    const trigger = screen.getByRole("button", { name: "Output type: Website" });
    expect(document.activeElement).toBe(trigger);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(submit).not.toHaveBeenCalled();
    await user.click(trigger);
    expect(screen.getByRole("menuitemradio", { name: "Website" }).getAttribute("aria-checked")).toBe("true");
  });

  it("supports arrows, type-ahead, Home, End, selection, and Escape", async () => {
    const user = userEvent.setup(); render(<Picker />);
    const trigger = screen.getByRole("button", { name: "Output type: Auto" });
    trigger.focus(); await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Auto" }));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Chat" }));
    await user.keyboard("w");
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Website" }));
    await user.keyboard("{End}");
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Image" }));
    await user.keyboard("{Home}{ArrowUp}");
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Image" }));
    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(trigger);
    expect(screen.queryByRole("menu")).toBeNull();
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    expect(screen.getByRole("button", { name: "Output type: Chat" })).toBeTruthy();
  });

  it("dismisses on outside clicks and Tab without trapping focus", async () => {
    const user = userEvent.setup(); render(<Picker />);
    await user.click(screen.getByRole("button", { name: "Output type: Auto" }));
    await user.tab();
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Send" }));
    await user.click(screen.getByRole("button", { name: "Output type: Auto" }));
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("labels deferred outputs honestly and preserves placeholder selection", async () => {
    const user = userEvent.setup(); render(<Picker />);
    await user.click(screen.getByRole("button", { name: "Output type: Auto" }));
    expect(screen.getByText("Coming later")).toBeTruthy();
    const deferred = screen.getByRole("menuitemradio", { name: "Image" });
    expect(deferred.getAttribute("aria-describedby")).toBeTruthy();
    await user.click(deferred);
    expect(screen.getByRole("button", { name: "Output type: Image" })).toBeTruthy();
  });
});
