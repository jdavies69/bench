// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ScopePanel, ago } from "./ScopePanel";

const nativeMock = vi.hoisted(() => ({
  loadScope: vi.fn(), setScopeOutcome: vi.fn(), setScopeAutoUpdate: vi.fn(), addScopeItem: vi.fn(), updateScopeItem: vi.fn(),
  resolveScopeChange: vi.fn(), scopeSessionPrompt: vi.fn(), reconcileScope: vi.fn(),
}));
vi.mock("../native", () => ({ native: nativeMock }));

const project = { id: "project-1", name: "Laundros", isSystem: false };
const item = (id: string, title: string, extra: object = {}) => ({ id, title, doneWhen: "", area: "", status: "todo", bucket: "scope", origin: "user", sessions: [], ...extra });
const change = (id: string, kind: string, title: string, extra: object = {}) => ({ id, kind, fit: "", itemId: null, title, area: "", doneWhen: "", reason: `Reason for ${title}`, conversationId: null, conversationTitle: null, createdAt: "2026-11-23 10:00:00", ...extra });
const scope = {
  projectId: "project-1", outcome: "Launch a booking site", autoUpdate: true, lastRunAt: "2026-11-23 10:00:00", lastRunStatus: "ok", stale: false,
  progress: { inScope: 2, done: 1, doing: 1, later: 1, dropped: 0, percent: 50, addedWeek: 2, doneWeek: 1, areas: [{ name: "Copy", done: 1, total: 1 }, { name: "Booking", done: 0, total: 1 }] },
  items: [
    item("item-1", "Set up booking form", { doneWhen: "A test booking arrives", area: "Booking", status: "doing", sessions: [{ conversationId: "chat-9", title: "Booking form session" }] }),
    item("item-2", "Write pricing page", { area: "Copy", status: "done" }),
    item("item-3", "Loyalty program", { bucket: "later" }),
  ],
  changes: [
    change("change-1", "add", "Choose an SMS provider", { fit: "needed", conversationId: "chat-9", conversationTitle: "Booking form session" }),
    change("change-2", "add", "Referral codes", { fit: "extra" }),
    change("change-3", "complete", "Set up booking form", { itemId: "item-1" }),
    change("change-4", "reconsider", "Loyalty program", { itemId: "item-3" }),
  ],
};
const handlers = () => ({ onClose: vi.fn(), onStart: vi.fn(), onOpen: vi.fn(), onChanged: vi.fn(), onError: vi.fn() });
const show = (props: ReturnType<typeof handlers>, busy = false) => render(<ScopePanel project={project} busy={busy} {...props} />);

beforeEach(() => { vi.resetAllMocks(); nativeMock.loadScope.mockResolvedValue(scope); });
afterEach(cleanup);

describe("Scope page", () => {
  it("starts from an outcome and drafts a list only when asked", async () => {
    const empty = { ...scope, lastRunAt: null, lastRunStatus: "", progress: { ...scope.progress, inScope: 0, done: 0, doing: 0, later: 0, percent: 0, addedWeek: 0, doneWeek: 0, areas: [] }, items: [], changes: [] };
    nativeMock.loadScope.mockResolvedValue(null); nativeMock.setScopeOutcome.mockResolvedValue(empty); nativeMock.reconcileScope.mockResolvedValue(empty);
    const props = handlers(); const user = userEvent.setup(); show(props);
    await screen.findByRole("heading", { name: "What does done look like?" });
    expect(screen.queryByRole("progressbar")).toBeNull();
    await user.type(screen.getByRole("textbox", { name: "Outcome" }), "Launch a booking site");
    await user.click(screen.getByRole("button", { name: "Set outcome" }));
    await waitFor(() => expect(nativeMock.setScopeOutcome).toHaveBeenCalledWith("project-1", "Launch a booking site"));
    await screen.findByText("Not updated yet");
    expect(screen.getByText("0 of 0 done")).toBeTruthy();
    expect(props.onChanged).toHaveBeenCalled();
    // Setting the outcome alone never contacts a model.
    expect(nativeMock.reconcileScope).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Draft a list from the outcome" }));
    await waitFor(() => expect(nativeMock.reconcileScope).toHaveBeenCalledWith("project-1", true));
  });

  it("shows progress, weekly flow, and per-area bars from native figures", async () => {
    show(handlers());
    const overall = await screen.findByRole("progressbar", { name: "Scope complete" });
    expect(overall.getAttribute("aria-valuenow")).toBe("50");
    expect(screen.getByText("1 of 2 done")).toBeTruthy();
    expect(screen.getByText("Last 7 days: 2 added · 1 done · 1 parked for later")).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Copy complete" }).getAttribute("aria-valuenow")).toBe("100");
    expect(screen.getByRole("progressbar", { name: "Booking complete" }).getAttribute("aria-valuenow")).toBe("0");
    expect(screen.getByText("Done when: A test booking arrives")).toBeTruthy();
    expect(screen.getByText("In progress")).toBeTruthy();
    // An up-to-date scope makes no request on open.
    expect(nativeMock.reconcileScope).not.toHaveBeenCalled();
  });

  it("applies exactly the reviewed suggestion and parks new ideas by default", async () => {
    nativeMock.resolveScopeChange.mockResolvedValue(scope);
    const props = handlers(); const user = userEvent.setup(); show(props);
    const changed = await screen.findByRole("region", { name: "Changed" });
    expect(within(changed).getByRole("heading", { name: "Changed · 4" })).toBeTruthy();
    expect(within(changed).getByText("Missing from scope")).toBeTruthy();
    expect(within(changed).getByText("Looks done")).toBeTruthy();
    expect(within(changed).getByText("Still needed?")).toBeTruthy();
    const idea = within(changed).getByText("New idea").parentElement!;
    expect(within(idea).getAllByRole("button").map((button) => button.textContent)).toEqual(["Later", "Add to scope", "Dismiss"]);
    for (const [name, id, decision] of [
      ["Add to scope: Choose an SMS provider", "change-1", "accept"], ["Later: Referral codes", "change-2", "later"],
      ["Not yet: Set up booking form", "change-3", "dismiss"], ["Keep: Loyalty program", "change-4", "dismiss"], ["Drop: Loyalty program", "change-4", "accept"],
    ]) {
      await user.click(within(changed).getByRole("button", { name }));
      await waitFor(() => expect(nativeMock.resolveScopeChange).toHaveBeenLastCalledWith(id, decision));
    }
    await user.click(within(changed).getByRole("button", { name: "From “Booking form session”" }));
    expect(props.onOpen).toHaveBeenCalledWith("chat-9");
  });

  it("kicks off a ready session from the saved item and waits while Bench is busy", async () => {
    nativeMock.scopeSessionPrompt.mockResolvedValue("Set up booking form\n\nProject outcome: Launch a booking site");
    const props = handlers(); const user = userEvent.setup(); const view = show(props, true);
    const start = await screen.findByRole("button", { name: "Start: Set up booking form" });
    expect((start as HTMLButtonElement).disabled).toBe(true);
    view.unmount(); show(props);
    await user.click(await screen.findByRole("button", { name: "Start: Set up booking form" }));
    await waitFor(() => expect(props.onStart).toHaveBeenCalledWith(scope.items[0], "Set up booking form\n\nProject outcome: Launch a booking site"));
    expect(nativeMock.scopeSessionPrompt).toHaveBeenCalledWith("item-1");
    // Parked and finished items cannot start work.
    expect(screen.queryByRole("button", { name: "Start: Loyalty program" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Start: Write pricing page" })).toBeNull();
  });

  it("moves items only through explicit choices", async () => {
    nativeMock.updateScopeItem.mockResolvedValue(scope); nativeMock.addScopeItem.mockResolvedValue(scope); nativeMock.setScopeAutoUpdate.mockResolvedValue({ ...scope, autoUpdate: false });
    const user = userEvent.setup(); show(handlers());
    await user.click(await screen.findByRole("checkbox", { name: "Done: Set up booking form" }));
    await waitFor(() => expect(nativeMock.updateScopeItem).toHaveBeenLastCalledWith("item-1", { status: "done" }));
    await user.click(screen.getByRole("button", { name: "Later: Set up booking form" }));
    await waitFor(() => expect(nativeMock.updateScopeItem).toHaveBeenLastCalledWith("item-1", { bucket: "later" }));
    await user.click(screen.getByText("Done · 1"));
    await user.click(screen.getByRole("checkbox", { name: "Done: Write pricing page" }));
    await waitFor(() => expect(nativeMock.updateScopeItem).toHaveBeenLastCalledWith("item-2", { status: "todo" }));
    await user.click(screen.getByText("Later · 1"));
    await user.click(screen.getByRole("button", { name: "Move to scope: Loyalty program" }));
    await waitFor(() => expect(nativeMock.updateScopeItem).toHaveBeenLastCalledWith("item-3", { bucket: "scope" }));
    await user.type(screen.getByRole("textbox", { name: "Add an item" }), "Add opening hours{Enter}");
    await waitFor(() => expect(nativeMock.addScopeItem).toHaveBeenCalledWith("project-1", "Add opening hours", "scope"));
    const toggle = screen.getByRole("switch", { name: "Keep scope updated" });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    await user.click(toggle);
    await waitFor(() => expect(nativeMock.setScopeAutoUpdate).toHaveBeenCalledWith("project-1", false));
    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("false"));
  });

  it("catches up on unreviewed work once and stays quiet when that check fails", async () => {
    nativeMock.loadScope.mockResolvedValueOnce({ ...scope, stale: true }).mockResolvedValue({ ...scope, stale: true, lastRunStatus: "failed" });
    nativeMock.reconcileScope.mockRejectedValue(new Error("OpenRouter is rate limiting requests. Retry in a moment."));
    const props = handlers(); const user = userEvent.setup(); show(props);
    await waitFor(() => expect(nativeMock.reconcileScope).toHaveBeenCalledWith("project-1", false));
    await screen.findByText("Last update failed");
    expect(props.onError).not.toHaveBeenCalled();
    expect(nativeMock.reconcileScope).toHaveBeenCalledTimes(1);
    // The list is still usable, and an explicit retry reports its failure.
    expect(screen.getByRole("checkbox", { name: "Done: Set up booking form" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Update now" }));
    await waitFor(() => expect(props.onError).toHaveBeenCalledWith("OpenRouter is rate limiting requests. Retry in a moment."));
    expect(nativeMock.reconcileScope).toHaveBeenLastCalledWith("project-1", true);
  });

  it.each([{ autoUpdate: false, lastRunStatus: "ok" }, { autoUpdate: true, lastRunStatus: "failed" }])("does not check automatically when $autoUpdate / $lastRunStatus", async (state) => {
    nativeMock.loadScope.mockResolvedValue({ ...scope, stale: true, ...state });
    show(handlers());
    await screen.findByText(state.lastRunStatus === "failed" ? "Last update failed" : "New work not reviewed yet");
    expect(nativeMock.reconcileScope).not.toHaveBeenCalled();
  });

  it("reports a load failure without offering to overwrite the outcome", async () => {
    nativeMock.loadScope.mockRejectedValue(new Error("Scope could not be loaded."));
    const props = handlers(); show(props);
    await screen.findByText("Scope could not be loaded.");
    expect(props.onError).toHaveBeenCalledWith("Scope could not be loaded.");
    expect(screen.queryByRole("textbox", { name: "Outcome" })).toBeNull();
  });

  it("describes update times from UTC timestamps", () => {
    const now = Date.parse("2026-11-23T12:00:00Z");
    expect(ago("2026-11-23 11:59:40", now)).toBe("just now");
    expect(ago("2026-11-23 11:15:00", now)).toBe("45 min ago");
    expect(ago("2026-11-23 07:00:00", now)).toBe("5 h ago");
    expect(ago("2026-11-20 12:00:00", now)).toBe("3 d ago");
    expect(ago("not a time", now)).toBe("");
  });
});
