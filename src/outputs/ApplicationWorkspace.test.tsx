// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ApplicationWorkspace, evaluateApplication } from "./ApplicationWorkspace";
import type { ArtifactState } from "../native";
const native = vi.hoisted(() => ({ loadApplicationValues: vi.fn(), saveApplicationValues: vi.fn() }));
vi.mock("../native", () => ({ native }));
const state: Extract<ArtifactState, { kind: "application" }> = { version: 1, kind: "application", revision: 2, requestCount: 1, content: { title: "Calculator", description: "A local calculator", fields: [{ id: "price", label: "Price", type: "number", default: 4, options: null }, { id: "enabled", label: "Enabled", type: "toggle", default: true, options: null }], outputs: [{ label: "Total", expression: { op: "multiply", left: { op: "input", id: "price" }, right: { op: "input", id: "enabled" } } }] } };
beforeEach(() => { vi.resetAllMocks(); native.loadApplicationValues.mockResolvedValue({ price: 7, enabled: true }); native.saveApplicationValues.mockImplementation(async (_id, _revision, values) => values); });
afterEach(cleanup);
it("loads persisted inputs, calculates interactively, and explicitly saves through Rust", async () => {
  render(<ApplicationWorkspace conversationId="app-1" state={state} busy={false} onExport={vi.fn()} />);
  const price = await screen.findByLabelText("Price"); expect((price as HTMLInputElement).value).toBe("7"); expect(screen.getByText("7", { selector: "output" })).toBeTruthy();
  fireEvent.change(price, { target: { value: "9" } }); expect(screen.getByText("9", { selector: "output" })).toBeTruthy(); expect(native.saveApplicationValues).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Save inputs" })); await screen.findByText("Inputs saved."); expect(native.saveApplicationValues).toHaveBeenCalledWith("app-1", 2, { price: 9, enabled: true });
  await userEvent.click(screen.getByLabelText("Enabled")); expect(screen.getByText("0", { selector: "output" })).toBeTruthy();
});
it("preserves changed inputs on failed save and offers load recovery without fake defaults", async () => {
  native.loadApplicationValues.mockRejectedValueOnce(new Error("secret")).mockResolvedValueOnce({ price: 7, enabled: true }); native.saveApplicationValues.mockRejectedValueOnce(new Error("private"));
  render(<ApplicationWorkspace conversationId="app-1" state={state} busy={false} onExport={vi.fn()} />); await screen.findByRole("alert"); expect(screen.queryByLabelText("Price")).toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Retry saved inputs" })); fireEvent.change(await screen.findByLabelText("Price"), { target: { value: "11" } });
  await userEvent.click(screen.getByRole("button", { name: "Save inputs" })); expect((await screen.findByRole("alert")).textContent).toBe("Couldn’t save inputs. Your changes are still here; try again."); expect((screen.getByLabelText("Price") as HTMLInputElement).value).toBe("11");
});
it("exports native standalone application and never executes descriptions as HTML", async () => {
  const onExport = vi.fn(); const { container } = render(<ApplicationWorkspace conversationId="app-1" state={{ ...state, content: { ...state.content, description: '<script>bad()</script>' } }} busy={false} onExport={onExport} />);
  await screen.findByLabelText("Price"); expect(container.querySelector("script")).toBeNull(); await userEvent.click(screen.getByRole("button", { name: "Export application" })); expect(onExport).toHaveBeenCalledTimes(1);
});
it("rejects blank numeric values, undefined math, oversized arithmetic, and expression depth", () => {
  expect(() => evaluateApplication({ op: "input", id: "price" }, { price: "" })).toThrow();
  expect(() => evaluateApplication({ op: "divide", left: { op: "constant", value: 1 }, right: { op: "constant", value: 0 } }, {})).toThrow();
  expect(() => evaluateApplication({ op: "multiply", left: { op: "constant", value: 1e12 }, right: { op: "constant", value: 2 } }, {})).toThrow();
  expect(() => evaluateApplication({ op: "constant", value: 1 }, {}, 13)).toThrow();
});
it.each([ ["add", 9], ["subtract", 5], ["multiply", 14], ["divide", 3.5], ["min", 2], ["max", 7] ] as const)("matches the fixed native interpreter for %s", (op, expected) => {
  expect(evaluateApplication({ op, left: { op: "input", id: "left" }, right: { op: "input", id: "right" } }, { left: 7, right: 2 })).toBe(expected);
});
