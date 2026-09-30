// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppUpdates } from "./AppUpdates";
const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
const ready = { status: "ready", version: "0.0.2", automatic: true, message: "Update ready." };
const idle = { status: "idle", version: null, automatic: true, message: "Ready to check." };
beforeEach(() => { vi.resetAllMocks(); native.invoke.mockResolvedValue(idle); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("shows a truthful unconfigured state without exposing working update actions", async () => {
  native.invoke.mockResolvedValue({ ...idle, status: "unconfigured", message: "Updates are not configured for this build." }); render(<AppUpdates />);
  await screen.findByText("Updates are not configured for this build."); expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
  expect(screen.getByRole("switch", { name: "Automatic updates" }).hasAttribute("disabled")).toBe(true); expect(native.invoke).toHaveBeenCalledExactlyOnceWith("app_update_status");
});
it("toggles automatic updates through native preference and refreshes its status", async () => {
  let automatic = true; native.invoke.mockImplementation(async (command, args) => { if (command === "set_automatic_updates") automatic = args.enabled; return { ...idle, automatic }; });
  render(<AppUpdates />); await screen.findByText("Ready to check."); await userEvent.click(screen.getByRole("switch", { name: "Automatic updates" }));
  expect(native.invoke).toHaveBeenCalledWith("set_automatic_updates", { enabled: false }); expect(screen.getByRole("switch", { name: "Automatic updates" }).getAttribute("aria-checked")).toBe("false");
});
it("checks explicitly, blocks restart with a draft, and never installs automatically", async () => {
  native.invoke.mockImplementation(async (command) => command === "check_app_update" ? ready : idle);
  const view = render(<AppUpdates blocked />); await screen.findByText("Ready to check."); await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));
  const restart = await screen.findByRole("button", { name: "Restart to update to 0.0.2" }); expect(restart.hasAttribute("disabled")).toBe(true);
  expect(screen.getByText(/save or clear your draft/)).toBeTruthy(); fireEvent.click(restart); expect(native.invoke).not.toHaveBeenCalledWith("install_app_update");
  view.rerender(<AppUpdates blocked={false} />); await userEvent.click(screen.getByRole("button", { name: "Restart to update to 0.0.2" })); expect(native.invoke).toHaveBeenCalledWith("install_app_update");
});
it("reports check/install failure safely and prevents duplicate operations", async () => {
  let fail: (e: Error) => void = () => {}; native.invoke.mockImplementation((command) => command === "check_app_update" ? new Promise((_, reject) => { fail = reject; }) : Promise.resolve(ready));
  render(<AppUpdates />); await screen.findByText("Update ready."); const check = screen.getByRole("button", { name: "Check for updates" }); fireEvent.click(check); fireEvent.click(check);
  expect(native.invoke.mock.calls.filter(([name]) => name === "check_app_update")).toHaveLength(1); await act(async () => fail(new Error("private endpoint detail")));
  expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t check for updates. Try again.");
  native.invoke.mockRejectedValueOnce(new Error("secret")); await userEvent.click(screen.getByRole("button", { name: "Restart to update to 0.0.2" }));
  expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t install the update. Your saved work is still here.");
});
it("allows retry when loading status fails and ignores callbacks after unmount", async () => {
  native.invoke.mockRejectedValueOnce(new Error("secret")).mockResolvedValueOnce(idle); const view = render(<AppUpdates />);
  await screen.findByRole("alert"); await userEvent.click(screen.getByRole("button", { name: "Retry update information" })); await screen.findByText("Ready to check.");
  let finish: (v: typeof ready) => void = () => {}; native.invoke.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  await userEvent.click(screen.getByRole("button", { name: "Check for updates" })); view.unmount(); await act(async () => finish(ready)); expect(screen.queryByText("Update ready.")).toBeNull();
});
