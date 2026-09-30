// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DocumentWorkspace, type DocumentState } from "./DocumentWorkspace";
afterEach(cleanup);
const state: DocumentState = { kind: "document", revision: 2, requestCount: 2, content: { title: "A quiet document", markdown: "## Section\n\nActual **content**." } };
it("renders a document and exports through its native callback", async () => {
  const onExport = vi.fn(); render(<DocumentWorkspace state={state} busy={false} onExport={onExport} />);
  expect(screen.getByRole("heading", { name: "A quiet document" })).toBeTruthy(); expect(screen.getByRole("heading", { name: "Section" })).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "Export document" })); expect(onExport).toHaveBeenCalledTimes(1);
});
it("does not execute untrusted HTML or load generated remote images", () => {
  const unsafe = { ...state, content: { title: "<script>bad()</script>", markdown: '<script>bad()</script>\n\n<img src="https://example.com/track">\n\n![tracking](https://example.com/track.png)\n\n[bad](javascript:alert(1))' } };
  const { container } = render(<DocumentWorkspace state={unsafe} busy={true} onExport={vi.fn()} />);
  expect(container.querySelector("script")).toBeNull(); expect(container.querySelector("img")).toBeNull(); expect(container.querySelector('a[href^="javascript:"]')).toBeNull(); expect(screen.getByRole("button", { name: "Export document" }).hasAttribute("disabled")).toBe(true);
});
it("keeps document edits local until save and returns to the saved revision", async () => {
  const onEdit = vi.fn(); const view = render(<DocumentWorkspace state={state} busy={false} onExport={vi.fn()} onEdit={onEdit} />);
  await userEvent.click(screen.getByRole("button", { name: "Edit document" }));
  const content = screen.getByLabelText("Document content"); await userEvent.clear(content); await userEvent.type(content, "Edited copy"); expect(onEdit).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Save edits" })); expect(onEdit).toHaveBeenCalledWith({ title: state.content.title, markdown: "Edited copy" });
  view.rerender(<DocumentWorkspace state={{ ...state, revision: 3, content: { ...state.content, markdown: "Edited copy" } }} busy={false} onExport={vi.fn()} onEdit={onEdit} />);
  expect(screen.getByText("Edited copy")).toBeTruthy(); expect(screen.queryByLabelText("Document content")).toBeNull();
});
