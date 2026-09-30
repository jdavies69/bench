// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AgentWorkspace } from "./AgentWorkspace";
import type { ArtifactState } from "../native";
afterEach(cleanup);
const state: Extract<ArtifactState, { kind: "agent" }> = { version: 1, kind: "agent", revision: 1, requestCount: 1, content: { title: "Saved report", markdown: "## Findings\n\nSaved conversation summary.", steps: [{ tool: "read_conversation", summary: "Read three saved messages." }, { tool: "draft_report", summary: "Drafted a concise report." }, { tool: "inspect_draft", summary: "Checked the draft." }] } };
it("shows saved local synthesis with real steps and native export", async () => {
  const onExport = vi.fn(); render(<AgentWorkspace state={state} busy={false} onExport={onExport} />);
  expect(screen.getByText("Local writing and synthesis from this saved conversation.")).toBeTruthy(); expect(screen.getByRole("heading", { name: "Findings" })).toBeTruthy();
  await userEvent.click(screen.getByText("Work completed · 3 steps")); expect(screen.getByText("Read three saved messages.")).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "Export report" })); expect(onExport).toHaveBeenCalledTimes(1);
});
it("blocks active report content and keeps last saved report visible while working", () => {
  const { container } = render(<AgentWorkspace state={{ ...state, content: { ...state.content, markdown: '<script>bad()</script>\n\n![track](https://example.com/image)\n\nLast saved copy.' } }} busy onExport={vi.fn()} />);
  expect(container.querySelector("script")).toBeNull(); expect(container.querySelector("img")).toBeNull(); expect(screen.getByText("Last saved copy.")).toBeTruthy(); expect(screen.getByRole("button", { name: "Export report" }).hasAttribute("disabled")).toBe(true);
});
