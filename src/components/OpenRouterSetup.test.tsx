// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OpenRouterSetup } from "./OpenRouterSetup";
afterEach(cleanup);
function setup(overrides = {}) {
  const callbacks = { onConnect: vi.fn().mockResolvedValue(undefined), onCancelConnect: vi.fn().mockResolvedValue(undefined), onDismiss: vi.fn(), ...overrides };
  const view = render(<OpenRouterSetup {...callbacks} />); return { callbacks, view };
}
function deferred() { let resolve: () => void = () => {}; let reject: (error: Error) => void = () => {}; const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
describe("OpenRouter browser onboarding", () => {
  it("connects without a manual key field or secret in the DOM", async () => {
    const { callbacks, view } = setup(); expect(view.container.querySelector("input")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" })); await screen.findByRole("status");
    expect(callbacks.onConnect).toHaveBeenCalledExactlyOnceWith(); expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
  });
  it("blocks duplicate connects and exposes cancellation while waiting", async () => {
    const pending = deferred(); const { callbacks } = setup({ onConnect: vi.fn().mockReturnValue(pending.promise) });
    const button = screen.getByRole("button", { name: "Connect OpenRouter" }); fireEvent.click(button); fireEvent.click(button);
    expect(callbacks.onConnect).toHaveBeenCalledTimes(1); expect(screen.getByRole("button", { name: "Waiting for OpenRouter…" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Set up later" }).hasAttribute("disabled")).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Cancel connection" })); expect(callbacks.onCancelConnect).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve()); expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByRole("button", { name: "Connect OpenRouter" }).hasAttribute("disabled")).toBe(false);
  });
  it("allows retry after cancellation and ignores stale errors from the previous browser flow", async () => {
    const old = deferred(); const fresh = deferred(); const { callbacks } = setup({ onConnect: vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise) });
    await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" })); await userEvent.click(screen.getByRole("button", { name: "Cancel connection" }));
    await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" })); await act(async () => old.reject(new Error("secret callback")));
    expect(screen.queryByRole("alert")).toBeNull(); expect(screen.getByRole("button", { name: "Waiting for OpenRouter…" })).toBeTruthy();
    await act(async () => fresh.resolve()); await screen.findByRole("status"); expect(callbacks.onConnect).toHaveBeenCalledTimes(2);
  });
  it("renders generic failures and ignores completion after unmount", async () => {
    setup({ onConnect: vi.fn().mockRejectedValue(new Error("secret token")) }); await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t connect to OpenRouter. Try again."); cleanup();
    const pending = deferred(); const { view } = setup({ onConnect: vi.fn().mockReturnValue(pending.promise) }); await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" })); view.unmount(); await act(async () => pending.resolve()); expect(screen.queryByRole("status")).toBeNull();
  });
  it("prevents duplicate cancel calls and keeps retry cancellation available after a failure", async () => {
    const pending = deferred(); const cancellation = deferred(); const { callbacks } = setup({ onConnect: vi.fn().mockReturnValue(pending.promise), onCancelConnect: vi.fn().mockReturnValueOnce(cancellation.promise).mockResolvedValueOnce(undefined) });
    await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" })); const button = screen.getByRole("button", { name: "Cancel connection" }); fireEvent.click(button); fireEvent.click(button);
    expect(callbacks.onCancelConnect).toHaveBeenCalledTimes(1); await act(async () => cancellation.reject(new Error("secret"))); expect((await screen.findByRole("alert")).textContent).not.toContain("secret");
    await userEvent.click(screen.getByRole("button", { name: "Cancel connection" })); expect(screen.getByRole("button", { name: "Connect OpenRouter" })).toBeTruthy(); await act(async () => pending.resolve());
  });
  it("allows setup later before connecting", async () => {
    const { callbacks } = setup(); await userEvent.click(screen.getByRole("button", { name: "Set up later" }));
    expect(callbacks.onDismiss).toHaveBeenCalledTimes(1); expect(callbacks.onConnect).not.toHaveBeenCalled();
  });
});

it("cancels a busy native flow on unmount using the latest callback without cancelling on rerender", async () => {
  const pending = deferred(); const firstCancel = vi.fn().mockResolvedValue(undefined); const latestCancel = vi.fn().mockResolvedValue(undefined);
  const { callbacks, view } = setup({ onConnect: vi.fn().mockReturnValue(pending.promise), onCancelConnect: firstCancel });
  await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" }));
  view.rerender(<OpenRouterSetup {...callbacks} onCancelConnect={latestCancel} />);
  expect(firstCancel).not.toHaveBeenCalled(); expect(latestCancel).not.toHaveBeenCalled();
  view.unmount(); expect(latestCancel).toHaveBeenCalledTimes(1); expect(firstCancel).not.toHaveBeenCalled();
  await act(async () => pending.resolve());
});
it("does not cancel when unmounted idle or duplicate a cancellation already in flight", async () => {
  const idle = setup(); idle.view.unmount(); expect(idle.callbacks.onCancelConnect).not.toHaveBeenCalled();
  const pending = deferred(); const cancellation = deferred();
  const active = setup({ onConnect: vi.fn().mockReturnValue(pending.promise), onCancelConnect: vi.fn().mockReturnValue(cancellation.promise) });
  await userEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" }));
  await userEvent.click(screen.getByRole("button", { name: "Cancel connection" }));
  active.view.unmount(); expect(active.callbacks.onCancelConnect).toHaveBeenCalledTimes(1);
  await act(async () => { cancellation.resolve(); pending.resolve(); });
});
