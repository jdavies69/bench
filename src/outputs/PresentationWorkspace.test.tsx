// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PresentationWorkspace, type PresentationState } from "./PresentationWorkspace";
afterEach(cleanup);
const state: PresentationState = { kind: "presentation", revision: 1, requestCount: 1, content: { title: "Deck", slides: [{ title: "First", body: "First **body**", notes: "Private speaker note" }, { title: "Second", body: "Second body", notes: "" }] } };
it("navigates within bounds with native buttons and keyboard", async () => {
  render(<PresentationWorkspace state={state} busy={false} onExport={vi.fn()} />);
  const reader = screen.getByRole("region", { name: "Presentation" });
  expect(screen.getByRole("button", { name: "Previous slide" }).hasAttribute("disabled")).toBe(true);
  await userEvent.click(screen.getByRole("button", { name: "Next slide" })); expect(screen.getByRole("heading", { name: "Second" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Next slide" }).hasAttribute("disabled")).toBe(true);
  fireEvent.keyDown(reader, { key: "ArrowRight" }); expect(screen.getByRole("status").textContent).toBe("2 / 2");
  fireEvent.keyDown(reader, { key: "Home" }); expect(screen.getByRole("heading", { name: "First" })).toBeTruthy();
  fireEvent.keyDown(reader, { key: "ArrowLeft" }); expect(screen.getByRole("status").textContent).toBe("1 / 2");
  fireEvent.keyDown(reader, { key: "End" }); expect(screen.getByRole("heading", { name: "Second" })).toBeTruthy();
});
it("shows notes only on request and exports through the callback", async () => {
  const onExport = vi.fn(); render(<PresentationWorkspace state={state} busy={false} onExport={onExport} />);
  expect(screen.queryByText("Private speaker note")).toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Show notes" })); expect(screen.getByText("Private speaker note")).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "Hide notes" })); expect(screen.queryByText("Private speaker note")).toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Export presentation" })); expect(onExport).toHaveBeenCalledTimes(1);
});
it("clamps navigation after a shorter revision and ignores modified keyboard shortcuts", () => {
  const view = render(<PresentationWorkspace state={state} busy={false} onExport={vi.fn()} />); const reader = screen.getByRole("region", { name: "Presentation" });
  fireEvent.keyDown(reader, { key: "End" }); view.rerender(<PresentationWorkspace state={{ ...state, revision: 2, content: { ...state.content, slides: [state.content.slides[0]] } }} busy={true} onExport={vi.fn()} />);
  expect(screen.getByRole("status").textContent).toBe("1 / 1"); expect(screen.getByRole("button", { name: "Export presentation" }).hasAttribute("disabled")).toBe(true);
  fireEvent.keyDown(reader, { key: "ArrowRight", metaKey: true }); expect(screen.getByRole("heading", { name: "First" })).toBeTruthy();
});
it("blocks raw scripts and remote Markdown images in slide bodies and notes", async () => {
  const unsafe = '<script>bad()</script>\n\n![tracking](https://example.com/track.png)';
  const { container } = render(<PresentationWorkspace state={{ ...state, content: { title: "Deck", slides: [{ title: "Title", body: unsafe, notes: unsafe }] } }} busy={false} onExport={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Show notes" })); expect(container.querySelector("script")).toBeNull(); expect(container.querySelector("img")).toBeNull();
});
it("edits slide content and notes without intercepting textarea navigation", async () => {
  const onEdit = vi.fn(); render(<PresentationWorkspace state={state} busy={false} onExport={vi.fn()} onEdit={onEdit} />);
  await userEvent.click(screen.getByRole("button", { name: "Edit presentation" })); const body = screen.getByLabelText("Slide content");
  fireEvent.keyDown(body, { key: "ArrowRight" }); expect(screen.getByRole("status").textContent).toBe("1 / 2");
  await userEvent.clear(body); await userEvent.type(body, "Edited slide"); await userEvent.click(screen.getByRole("button", { name: "Next slide" }));
  await userEvent.clear(screen.getByLabelText("Speaker notes")); await userEvent.type(screen.getByLabelText("Speaker notes"), "New notes");
  expect(onEdit).not.toHaveBeenCalled(); await userEvent.click(screen.getByRole("button", { name: "Save edits" }));
  expect(onEdit).toHaveBeenCalledWith({ title: "Deck", slides: [{ ...state.content.slides[0], body: "Edited slide" }, { ...state.content.slides[1], notes: "New notes" }] });
});
