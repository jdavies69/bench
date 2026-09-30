// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";

const nativeMock = vi.hoisted(() => ({
  snapshot: vi.fn(), messages: vi.fn(), toolActivity: vi.fn(), createMessage: vi.fn(), stream: vi.fn(), loadWebsite: vi.fn(),
  websiteRevisions: vi.fn(), generateWebsite: vi.fn(), restoreWebsite: vi.fn(), prepareWebsiteAction: vi.fn(), approveAction: vi.fn(),
  configureWebSearch: vi.fn(),
}));
vi.mock("./native", () => ({ native: nativeMock }));

const conversation = { id: "site-1", projectId: "miscellaneous", title: "Build a website", outputType: "website", outputSelection: "auto", updatedAt: "today" };
const userMessage = { id: "message-1", conversationId: "site-1", role: "user", content: "Build a website", createdAt: "today" };
const site = { pages: [{ path: "index.html", html: "<h1>Local site</h1>" }], css: "h1 { color: black; }", revision: 1, requestCount: 1 };
const snapshot = {
  outputs: [{ id: "chat", label: "Chat", implemented: true, workspace: "conversation" }, { id: "website", label: "Website", implemented: true, workspace: "canvas" }],
  projects: [{ id: "miscellaneous", name: "Miscellaneous", isSystem: true }], conversations: [],
  settings: { executionBehavior: "discuss", approvalBehavior: "always", modelProvider: "openrouter", sidebarCollapsed: false, webSearchBackend: "auto", webSearchUrl: "" }, providers: [], webSearchKeySource: "none", webSearchStatus: "unavailable",
};

beforeEach(() => {
  vi.resetAllMocks();
  Element.prototype.scrollIntoView = vi.fn();
  nativeMock.snapshot.mockResolvedValue(snapshot);
  nativeMock.messages.mockResolvedValue([userMessage]);
  nativeMock.toolActivity.mockResolvedValue([]);
  nativeMock.createMessage.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [conversation] }); return conversation; });
  nativeMock.loadWebsite.mockResolvedValue(null);
  nativeMock.prepareWebsiteAction.mockResolvedValue({ id: "review-1", conversationId: "site-1", title: "Create this website?", detail: "Create a static website from your saved request and open its preview." });
  nativeMock.approveAction.mockResolvedValue("one-use-grant");
  nativeMock.generateWebsite.mockResolvedValue(site);
  nativeMock.websiteRevisions.mockResolvedValue([{ revision: 1, current: true }]);
  nativeMock.configureWebSearch.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("Web search settings", () => {
  it("shows OpenRouter availability and turns search off without exposing separate credentials", async () => {
    const available = { ...snapshot, webSearchStatus: "openrouter" };
    nativeMock.snapshot.mockResolvedValueOnce(available).mockResolvedValueOnce({ ...available, webSearchStatus: "off", settings: { ...available.settings, webSearchBackend: "off" } });
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByText("Available with OpenRouter")).toBeTruthy();
    expect(screen.queryByText("Brave")).toBeNull();
    expect(screen.queryByText("SearXNG")).toBeNull();
    const toggle = screen.getByRole("switch", { name: "Web search" });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    await user.click(toggle);
    await waitFor(() => expect(nativeMock.configureWebSearch).toHaveBeenCalledWith("off", ""));
    await screen.findByText("Off");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });

  it("shows unavailable when OpenRouter is not the selected, connected provider", async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Web search" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.queryByLabelText("Brave Search API key")).toBeNull();
  });
});

describe("Website authorization flow", () => {
  it("saves the prompt and waits for explicit approval before generation", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Build a website");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Create this website?");
    expect(nativeMock.createMessage).toHaveBeenCalledWith(null, "Build a website", "auto");
    expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(nativeMock.generateWebsite).toHaveBeenCalledWith("site-1", "one-use-grant"));
    const preview = await screen.findByTitle("Website preview");
    expect(preview.getAttribute("sandbox")).toBe("");
    expect(preview.getAttribute("srcdoc")).toContain("Local site");
    expect(nativeMock.approveAction).toHaveBeenCalledWith("review-1");
  });

  it("keeps a declined request and can review it again without duplicating the prompt", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Build a website");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Create this website?");
    await user.click(screen.getByRole("button", { name: "Not now" }));
    expect(screen.getByText("Request saved. Continue when ready.")).toBeTruthy();
    expect(screen.getAllByText("Build a website").length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByText("Create this website?");
    expect(nativeMock.createMessage).toHaveBeenCalledTimes(1);
    expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
  });

  it("updates a committed preview even if loading version history fails", async () => {
    nativeMock.prepareWebsiteAction.mockResolvedValue(null);
    nativeMock.websiteRevisions.mockRejectedValue(new Error("disk read error"));
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Build a website");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    const preview = await screen.findByTitle("Website preview");
    expect(preview.getAttribute("srcdoc")).toContain("Local site");
    expect(await screen.findByText("Website saved. Version history is temporarily unavailable.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
  });

  it("does not let a slow conversation load overwrite the selected conversation", async () => {
    const other = { ...conversation, id: "chat-2", title: "Other chat", outputType: "chat" };
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [conversation, other] });
    let finishOldLoad: (value: unknown[]) => void = () => {};
    const oldLoad = new Promise<unknown[]>((resolve) => { finishOldLoad = resolve; });
    nativeMock.messages.mockImplementation((id: string) => id === "site-1" ? oldLoad : Promise.resolve([{ ...userMessage, id: "reply-2", conversationId: "chat-2", role: "assistant", content: "Other reply" }]));
    nativeMock.loadWebsite.mockImplementation(async (id: string) => id === "site-1" ? site : null);
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Build a website" }));
    await user.click(screen.getByRole("button", { name: "Other chat" }));
    await screen.findByText("Other reply");
    await act(async () => { finishOldLoad([{ ...userMessage, content: "Old request" }]); });
    expect(screen.queryByText("Old request")).toBeNull();
    expect(screen.queryByTitle("Website preview")).toBeNull();
    expect(screen.getByText("Other reply")).toBeTruthy();
  });

  it("keeps an unanswered revision available to Retry after restoring a saved version", async () => {
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [conversation] });
    nativeMock.messages.mockResolvedValue([userMessage, { ...userMessage, id: "message-2", content: "Make the header smaller" }]);
    nativeMock.loadWebsite.mockResolvedValue({ ...site, revision: 2 });
    nativeMock.websiteRevisions.mockResolvedValue([{ revision: 1, current: false }, { revision: 2, current: true }]);
    nativeMock.prepareWebsiteAction.mockResolvedValue(null);
    nativeMock.restoreWebsite.mockResolvedValue(site);
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Build a website" }));
    await screen.findByText("Response not completed.");
    await user.click(screen.getByText("Version 2"));
    await user.click(screen.getByRole("button", { name: "Restore version 1" }));
    await waitFor(() => expect(nativeMock.restoreWebsite).toHaveBeenCalledWith("site-1", 1, undefined));
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(nativeMock.createMessage).not.toHaveBeenCalled();
  });
});
