// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OpenRouterSetup } from "./OpenRouterSetup";

afterEach(cleanup);
function setup(overrides = {}) {
  const callbacks = { onConnect: vi.fn().mockResolvedValue(undefined), onOpenKeys: vi.fn().mockResolvedValue(undefined), onDismiss: vi.fn(), onOtherProviders: vi.fn(), ...overrides };
  render(<OpenRouterSetup {...callbacks} />); return callbacks;
}
const keyField = () => screen.getByLabelText("Already have a key? Paste it here.") as HTMLInputElement;

describe("OpenRouter onboarding", () => {
  it("focuses an immediate password input and rejects a blank key without a request", async () => {
    const callbacks = setup(); expect(keyField().type).toBe("password"); expect(document.activeElement).toBe(keyField());
    fireEvent.submit(keyField().closest("form")!);
    expect(await screen.findByRole("alert")).toBeTruthy(); expect(callbacks.onConnect).not.toHaveBeenCalled();
  });
  it("trims once, prevents duplicate submissions, and clears only after successful save", async () => {
    let finish: () => void = () => {};
    const pending = new Promise<void>((resolve) => { finish = resolve; });
    const callbacks = setup({ onConnect: vi.fn().mockReturnValue(pending) }); const user = userEvent.setup();
    await user.type(keyField(), "  secret-key  ");
    fireEvent.submit(keyField().closest("form")!); fireEvent.submit(keyField().closest("form")!);
    expect(callbacks.onConnect).toHaveBeenCalledExactlyOnceWith("secret-key"); expect(keyField().value).toBe("  secret-key  ");
    expect(screen.getByRole("button", { name: "Set up later" }).hasAttribute("disabled")).toBe(true);
    finish(); await screen.findByRole("status"); expect(keyField().value).toBe(""); expect(screen.queryByText("secret-key")).toBeNull();
  });
  it("preserves a failed key for retry and hides native error details", async () => {
    const callbacks = setup({ onConnect: vi.fn().mockRejectedValueOnce(new Error("secret-key rejected by storage")).mockResolvedValueOnce(undefined) }); const user = userEvent.setup();
    await user.type(keyField(), "secret-key"); await user.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByRole("alert")).toBeTruthy(); expect(screen.getByRole("alert").textContent).not.toContain("secret-key"); expect(keyField().value).toBe("secret-key");
    await user.click(screen.getByRole("button", { name: "Connect" })); await screen.findByRole("status"); expect(callbacks.onConnect).toHaveBeenCalledTimes(2); expect(keyField().value).toBe("");
  });
  it("opens only the native supplied keys action and offers dismissal or other providers", async () => {
    const callbacks = setup(); const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Open OpenRouter ↗" })); await waitFor(() => expect(callbacks.onOpenKeys).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "Set up later" })); await user.click(screen.getByRole("button", { name: "Use another provider" }));
    expect(callbacks.onDismiss).toHaveBeenCalledTimes(1); expect(callbacks.onOtherProviders).toHaveBeenCalledTimes(1); expect(callbacks.onConnect).not.toHaveBeenCalled();
  });
  it("does not expose external-opening error details", async () => {
    setup({ onOpenKeys: vi.fn().mockRejectedValue(new Error("private path")) }); const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Open OpenRouter ↗" })); expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t open OpenRouter. Try again.");
  });
});
