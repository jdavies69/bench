// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OpenRouterUsage, type OpenRouterUsageData } from "./OpenRouterUsage";
const usage: OpenRouterUsageData = { usageDaily: 0, usageWeekly: 2, usageMonthly: 3.25, usageTotal: 10, limit: null, limitRemaining: null, limitReset: null, byokUsageMonthly: null };
afterEach(cleanup);
const callbacks = () => ({ connected: true, loadUsage: vi.fn().mockResolvedValue(usage), onManageCredits: vi.fn().mockResolvedValue(undefined) });
describe("OpenRouter usage", () => {
  it("only checks on demand and distinguishes actual zero, unlimited, and unavailable", async () => {
    const props = callbacks(); const view = render(<OpenRouterUsage {...props} />);
    expect(props.loadUsage).not.toHaveBeenCalled(); expect(screen.queryByText("$0.00")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Check usage" }));
    await screen.findByText("$0.00"); expect(screen.getByText("Unlimited")).toBeTruthy(); expect(screen.getByText("Not applicable")).toBeTruthy();
    props.loadUsage.mockResolvedValue({ ...usage, limit: 20, limitRemaining: null });
    await userEvent.click(screen.getByRole("button", { name: "Refresh usage" })); await screen.findByText("Unavailable");
    view.rerender(<OpenRouterUsage {...props} connected={false} />);
    expect(screen.getByText("Connect OpenRouter to see usage.")).toBeTruthy(); expect(screen.queryByText("$0.00")).toBeNull();
  });
  it("blocks duplicate clicks and preserves clearly stale figures after refresh failure", async () => {
    let finish: (value: OpenRouterUsageData) => void = () => {};
    const props = callbacks(); props.loadUsage.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    render(<OpenRouterUsage {...props} />);
    const button = screen.getByRole("button", { name: "Check usage" }); fireEvent.click(button); fireEvent.click(button); expect(props.loadUsage).toHaveBeenCalledTimes(1);
    await act(async () => finish(usage)); await screen.findByText("$3.25");
    props.loadUsage.mockRejectedValueOnce(new Error("secret-key provider error"));
    await userEvent.click(screen.getByRole("button", { name: "Refresh usage" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t check usage. Try again.");
    expect(screen.getByText("$3.25")).toBeTruthy(); expect(screen.getByText(/These figures may be out of date/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Refresh usage" }));
    expect(screen.queryByText(/These figures may be out of date/)).toBeNull();
  });
  it("ignores a late response after the connection changes", async () => {
    let finish: (value: OpenRouterUsageData) => void = () => {};
    const props = callbacks(); props.loadUsage.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const view = render(<OpenRouterUsage {...props} connectionVersion={1} />);
    await userEvent.click(screen.getByRole("button", { name: "Check usage" }));
    view.rerender(<OpenRouterUsage {...props} connectionVersion={2} />);
    await act(async () => finish(usage)); expect(screen.queryByText("$3.25")).toBeNull();
    expect(screen.getByRole("button", { name: "Check usage" }).hasAttribute("disabled")).toBe(false);
  });
  it("ignores a late response after unmount and safely handles management failure", async () => {
    let finish: (value: OpenRouterUsageData) => void = () => {};
    const props = callbacks(); props.loadUsage.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const view = render(<OpenRouterUsage {...props} />); await userEvent.click(screen.getByRole("button", { name: "Check usage" })); view.unmount(); await act(async () => finish(usage));
    render(<OpenRouterUsage {...callbacks()} onManageCredits={vi.fn().mockRejectedValue(new Error("secret"))} />);
    await userEvent.click(screen.getByRole("button", { name: "Manage credits" })); expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t open OpenRouter. Try again.");
  });
  it("rejects malformed usage without showing invented metrics", async () => {
    const props = callbacks(); props.loadUsage.mockResolvedValue({ ...usage, usageDaily: Number.NaN }); render(<OpenRouterUsage {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Check usage" })); await screen.findByRole("alert"); expect(screen.queryByText("$3.25")).toBeNull();
  });
});
