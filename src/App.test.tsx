// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";

const nativeMock = vi.hoisted(() => ({
  snapshot: vi.fn(), messages: vi.fn(), toolActivity: vi.fn(), createMessage: vi.fn(), stream: vi.fn(), loadWebsite: vi.fn(),
  websiteRevisions: vi.fn(), generateWebsite: vi.fn(), restoreWebsite: vi.fn(), prepareWebsiteAction: vi.fn(), approveAction: vi.fn(),
  chooseAttachments: vi.fn(), removeStagedAttachment: vi.fn(),
  configureWebSearch: vi.fn(), loadArtifact: vi.fn(), generateArtifact: vi.fn(), generateMediaOutput: vi.fn(), generateAgentOutput: vi.fn(), artifactRevisions: vi.fn(), restoreArtifact: vi.fn(), prepareArtifactAction: vi.fn(), exportArtifact: vi.fn(), saveArtifactEdits: vi.fn(),
  saveProviderKey: vi.fn(), openOpenRouterSetup: vi.fn(), connectOpenRouter: vi.fn(), cancelOpenRouterConnect: vi.fn(),
  openOpenRouterUsage: vi.fn(), openOpenRouterBilling: vi.fn(),
  openRouterUsage: vi.fn(),
}));
vi.mock("./native", () => ({ native: nativeMock }));

const conversation = { id: "site-1", projectId: "miscellaneous", title: "Build a website", outputType: "website", outputSelection: "auto", updatedAt: "today" };
const userMessage = { id: "message-1", conversationId: "site-1", role: "user", content: "Build a website", createdAt: "today" };
const site = { pages: [{ path: "index.html", html: "<h1>Local site</h1>" }], css: "h1 { color: black; }", revision: 1, requestCount: 1 };
const snapshot = {
  outputs: [{ id: "chat", label: "Chat", implemented: true, workspace: "conversation" }, { id: "website", label: "Website", implemented: true, workspace: "canvas" }],
  projects: [{ id: "miscellaneous", name: "Miscellaneous", isSystem: true }], conversations: [],
  settings: { executionBehavior: "discuss", approvalBehavior: "always", modelProvider: "openrouter", sidebarCollapsed: false, webSearchBackend: "auto", webSearchUrl: "" }, providers: [{ id: "openrouter", label: "OpenRouter", model: "test-model", keySource: "keychain" }], webSearchKeySource: "none", webSearchStatus: "unavailable",
};

beforeEach(() => {
  vi.resetAllMocks();
  Element.prototype.scrollIntoView = vi.fn();
  nativeMock.snapshot.mockResolvedValue(snapshot);
  nativeMock.messages.mockResolvedValue([userMessage]);
  nativeMock.toolActivity.mockResolvedValue([]);
  nativeMock.createMessage.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [conversation] }); return conversation; });
  nativeMock.loadWebsite.mockResolvedValue(null);
  nativeMock.loadArtifact.mockResolvedValue(null);
  nativeMock.artifactRevisions.mockResolvedValue([{ revision: 1, current: true }]);
  nativeMock.prepareArtifactAction.mockResolvedValue(null);
  nativeMock.exportArtifact.mockResolvedValue(true);
  nativeMock.prepareWebsiteAction.mockResolvedValue({ id: "review-1", conversationId: "site-1", title: "Create this website?", detail: "Create a static website from your saved request and open its preview." });
  nativeMock.approveAction.mockResolvedValue("one-use-grant");
  nativeMock.generateWebsite.mockResolvedValue(site);
  nativeMock.websiteRevisions.mockResolvedValue([{ revision: 1, current: true }]);
  nativeMock.configureWebSearch.mockResolvedValue(undefined);
  nativeMock.chooseAttachments.mockResolvedValue([]);
  nativeMock.removeStagedAttachment.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("Web search settings", () => {
  it("offers real provider usage and billing links with honest manual-update information", async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Usage & billing" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Data & privacy" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Automatic updates" })).toBeTruthy();
    expect(nativeMock.openRouterUsage).not.toHaveBeenCalled();
    nativeMock.openRouterUsage.mockResolvedValue({ usageDaily: 0, usageWeekly: 0.1, usageMonthly: 1.5, usageTotal: 2, limit: null, limitRemaining: null, limitReset: null, byokUsageMonthly: null });
    await user.click(screen.getByRole("button", { name: "Check usage" }));
    await waitFor(() => expect(nativeMock.openRouterUsage).toHaveBeenCalledWith());
    await screen.findByText("$1.50");
    expect(screen.getByText("$0.00")).toBeTruthy();
    expect(screen.getByText("Unlimited")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Manage credits" }));
    await waitFor(() => expect(nativeMock.openOpenRouterBilling).toHaveBeenCalledWith());
    expect(nativeMock.saveProviderKey).not.toHaveBeenCalled();
    expect(nativeMock.stream).not.toHaveBeenCalled();
  });
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

describe("Fresh install and input safety", () => {
  it("offers setup on a fresh disconnected install, opens native help, and connects without inference", async () => {
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, providers: [{ ...snapshot.providers[0], keySource: "none" }] });
    nativeMock.connectOpenRouter.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue(snapshot); });
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("heading", { name: "Connect OpenRouter" });
    await user.click(screen.getByRole("button", { name: "Connect OpenRouter" }));
    await screen.findByRole("textbox", { name: "Message" });
    expect(nativeMock.connectOpenRouter).toHaveBeenCalledWith();
    expect(nativeMock.saveProviderKey).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("OpenRouter API key")).toBeNull();
    expect(nativeMock.createMessage).not.toHaveBeenCalled();
    expect(nativeMock.stream).not.toHaveBeenCalled();
    expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
  });
  it("ignores a cancelled browser connection completing after a newer attempt starts", async () => {
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, providers: [{ ...snapshot.providers[0], keySource: "none" }] });
    let finishOld!: () => void;
    const old = new Promise<void>((resolve) => { finishOld = resolve; });
    nativeMock.connectOpenRouter.mockReturnValueOnce(old).mockReturnValueOnce(new Promise(() => {}));
    nativeMock.cancelOpenRouterConnect.mockResolvedValue(undefined);
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Connect OpenRouter" }));
    await user.click(screen.getByRole("button", { name: "Cancel connection" }));
    await user.click(await screen.findByRole("button", { name: "Connect OpenRouter" }));
    const reads = nativeMock.snapshot.mock.calls.length;
    await act(async () => { finishOld(); await old; });
    expect(nativeMock.snapshot).toHaveBeenCalledTimes(reads);
    expect(screen.getByRole("button", { name: "Waiting for OpenRouter…" })).toBeTruthy();
    expect(nativeMock.stream).not.toHaveBeenCalled();
  });
  it("keeps existing connected users in their workspace and offers setup from Settings", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("textbox", { name: "Message" });
    expect(screen.queryByRole("heading", { name: "Connect OpenRouter" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: /OpenRouter\s*Connected/ }));
    await user.click(screen.getByRole("button", { name: "Connect OpenRouter" }));
    await screen.findByRole("heading", { name: "Connect OpenRouter" });
    await user.click(screen.getByRole("button", { name: "Set up later" }));
    await screen.findByRole("heading", { name: "Settings" });
    expect(nativeMock.saveProviderKey).not.toHaveBeenCalled();
  });
  it("shows only OpenRouter settings while retaining legacy provider metadata", async () => {
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, providers: [snapshot.providers[0], ...["openai", "anthropic", "xai"].map((id) => ({ id, label: id === "openai" ? "OpenAI" : id === "anthropic" ? "Anthropic" : "xAI / Grok", model: "existing-model", keySource: "keychain" }))] });
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("button", { name: /OpenRouter\s*Connected/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /OpenAI|Anthropic|xAI/ })).toBeNull();
    await user.click(screen.getByText("Model IDs"));
    expect(screen.getByLabelText("OpenRouter")).toBeTruthy();
    expect(screen.queryByLabelText("OpenAI")).toBeNull();
    expect(screen.queryByLabelText("Anthropic")).toBeNull();
    expect(screen.queryByLabelText("xAI / Grok")).toBeNull();
    expect(nativeMock.saveProviderKey).not.toHaveBeenCalled();
  });
  it("saves the request before connection setup without making a model call", async () => {
    const disconnected = { ...snapshot, providers: [{ ...snapshot.providers[0], keySource: "none" }] };
    nativeMock.snapshot.mockResolvedValue(disconnected);
    nativeMock.createMessage.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue({ ...disconnected, conversations: [conversation] }); return conversation; });
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Set up later" }));
    await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Keep this idea");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByRole("heading", { name: "Connect OpenRouter" });
    expect(screen.getByRole("button", { name: "Connect OpenRouter" })).toBeTruthy();
    expect(nativeMock.createMessage).toHaveBeenCalledWith(null, "Keep this idea", "auto");
    expect(nativeMock.stream).not.toHaveBeenCalled();
    expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Set up later" }));
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    cleanup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Build a website" }));
    await screen.findByText("Response not completed.");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(nativeMock.createMessage).toHaveBeenCalledTimes(1);
  });
  it("keeps an unanswered saved request when Retry needs a connection", async () => {
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [conversation], providers: [{ ...snapshot.providers[0], keySource: "none" }] });
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Build a website" }));
    await screen.findByText("Response not completed.");
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByRole("heading", { name: "Connect OpenRouter" });
    expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
    expect(nativeMock.createMessage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Set up later" }));
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.getAllByText("Build a website").length).toBeGreaterThan(0);
  });
  it("does not submit Enter while an input method is composing", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("button", { name: "Output type: Auto" });
    const input = screen.getByRole("textbox", { name: "Message" });
    await user.type(input, "Draft");
    fireEvent.keyDown(input, { key: "Enter", isComposing: true, keyCode: 229 });
    expect(nativeMock.createMessage).not.toHaveBeenCalled();
    expect((input as HTMLTextAreaElement).value).toBe("Draft");
  });
});

const documentArtifact = { version: 1, kind: "document", revision: 1, requestCount: 1, content: { title: "Local brief", markdown: "A saved **brief**" } };
describe("Artifact workflows", () => {
  it.each(["document", "presentation", "image", "voice"])("saves and authorizes %s generation through its native provider", async (kind) => {
    const created = { ...conversation, id: "artifact-1", title: "Make an output", outputType: kind, outputSelection: kind };
    const artifact = kind === "document" ? documentArtifact : kind === "presentation" ? { ...documentArtifact, kind, content: { title: "Local deck", slides: [{ title: "First slide", body: "Body", notes: "" }] } } : { ...documentArtifact, kind, content: { mimeType: kind === "image" ? "image/png" : "audio/mpeg", dataBase64: "aGVsbG8=", model: "fixture", generationId: "fixture", prompt: "Saved request", voice: null } };
    nativeMock.createMessage.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); return created; });
    nativeMock.prepareArtifactAction.mockResolvedValue({ id: "artifact-review", conversationId: created.id, title: "Create this output?", detail: "Save an immutable output." });
    nativeMock.generateArtifact.mockResolvedValue(artifact); nativeMock.generateMediaOutput.mockResolvedValue(artifact);
    const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Make an output"); await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Create this output?"); expect(nativeMock.generateArtifact).not.toHaveBeenCalled(); expect(nativeMock.generateMediaOutput).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    const generation = kind === "image" || kind === "voice" ? nativeMock.generateMediaOutput : nativeMock.generateArtifact;
    await waitFor(() => expect(generation).toHaveBeenCalledWith(created.id, "one-use-grant"));
    await user.click(await screen.findByRole("button", { name: kind === "document" ? "Export document" : kind === "presentation" ? "Export presentation" : kind === "image" ? "Export image" : "Export audio" }));
    await waitFor(() => expect(nativeMock.exportArtifact).toHaveBeenCalledWith(created.id, kind === "image" || kind === "voice" ? "binary" : "text"));
  });
  it("loads a saved document after restart and preserves it through a failed revision", async () => {
    const created = { ...conversation, outputType: "document", outputSelection: "document", title: "Saved document" };
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); nativeMock.loadArtifact.mockResolvedValue(documentArtifact);
    nativeMock.messages.mockResolvedValue([userMessage, { ...userMessage, id: "message-2", content: "Revise the brief" }]); nativeMock.generateArtifact.mockRejectedValue(new Error("provider failed"));
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", { name: "Saved document" }));
    await screen.findByRole("heading", { name: "Local brief" }); expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Retry" })); await waitFor(() => expect(nativeMock.generateArtifact).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("heading", { name: "Local brief" })).toBeTruthy(); expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy(); expect(nativeMock.createMessage).not.toHaveBeenCalled();
  });
  it("restores a saved artifact version without consuming an unanswered revision", async () => {
    const created = { ...conversation, outputType: "document", title: "Saved document" };
    nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); nativeMock.loadArtifact.mockResolvedValue({ ...documentArtifact, revision: 2 });
    nativeMock.messages.mockResolvedValue([userMessage, { ...userMessage, id: "message-2", content: "Pending revision" }]);
    nativeMock.artifactRevisions.mockResolvedValue([{ revision: 1, current: false }, { revision: 2, current: true }]); nativeMock.restoreArtifact.mockResolvedValue(documentArtifact);
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", { name: "Saved document" }));
    await user.click(await screen.findByLabelText("Output versions")); await user.click(screen.getByRole("button", { name: "Restore version 1" }));
    await waitFor(() => expect(nativeMock.restoreArtifact).toHaveBeenCalledWith(created.id, 1, undefined));
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy(); expect(screen.getByRole("heading", { name: "Local brief" })).toBeTruthy();
  });
});

it("requires exact approval before Agent tools and loads the committed local report", async () => {
  const created = { ...conversation, id: "agent-1", title: "Synthesize a report", outputType: "agent", outputSelection: "agent" };
  const artifact = { ...documentArtifact, kind: "agent", content: { ...documentArtifact.content, steps: [{ tool: "read_conversation", summary: "Read saved content" }, { tool: "draft_report", summary: "Drafted local report" }] } };
  nativeMock.createMessage.mockImplementation(async () => { nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); return created; });
  nativeMock.prepareArtifactAction.mockResolvedValue({ id: "agent-review", conversationId: created.id, title: "Run local synthesis?", detail: "Read saved messages and write a local report." });
  nativeMock.generateAgentOutput.mockResolvedValue(artifact);
  const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
  await user.type(screen.getByRole("textbox", { name: "Message" }), "Synthesize a report"); await user.click(screen.getByRole("button", { name: "Send message" })); await screen.findByText("Run local synthesis?");
  expect(nativeMock.generateAgentOutput).not.toHaveBeenCalled(); await user.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() => expect(nativeMock.generateAgentOutput).toHaveBeenCalledWith(created.id, "one-use-grant")); await screen.findByRole("heading", { name: "Local brief" });
  await user.click(screen.getByRole("button", { name: "Export report" })); await waitFor(() => expect(nativeMock.exportArtifact).toHaveBeenCalledWith(created.id, "text"));
});

it.each([
  "OpenRouter could not fund this media request. Check your credits or key spending limit.",
  "This media request is too large. Shorten the prompt or start a new conversation.",
  "OpenRouter rejected this media request. Try a different prompt.",
  "This media model is currently unavailable on OpenRouter. Try again later.",
  "OpenRouter media generation is temporarily unavailable. Try again later.",
  "Generated media exceeds the size limit.",
])("preserves the safe media recovery reason: %s", async (reason) => {
  const created = { ...conversation, id: "image-1", title: "A beach day in nyc", outputType: "image", outputSelection: "image" };
  nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); nativeMock.messages.mockResolvedValue([{ ...userMessage, content: created.title }]); nativeMock.generateMediaOutput.mockRejectedValue(new Error(reason));
  const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", { name: created.title })); await user.click(await screen.findByRole("button", { name: "Retry" }));
  await screen.findByText(reason); expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy(); expect(nativeMock.createMessage).not.toHaveBeenCalled();
});
it("does not expose provider details appended to a known media error", async () => {
  const created = { ...conversation, outputType: "image", title: "Saved image request" };
  nativeMock.snapshot.mockResolvedValue({ ...snapshot, conversations: [created] }); nativeMock.generateMediaOutput.mockRejectedValue(new Error("Could not reach image generation. secret-provider-token"));
  const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", { name: created.title })); await user.click(await screen.findByRole("button", { name: "Retry" }));
  await screen.findByText("Couldn’t complete the response."); expect(screen.queryByText(/secret-provider-token/)).toBeNull();
});

describe("Native attachments", () => {
  const file = { id: "attachment-1", name: "brief.txt", bytes: 24 };
  it("keeps the draft when the picker is cancelled and makes no model call", async () => {
    const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Keep this draft"); await user.click(screen.getByRole("button", { name: "Add files" }));
    expect(nativeMock.chooseAttachments).toHaveBeenCalledTimes(1); expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toBe("Keep this draft"); expect(nativeMock.createMessage).not.toHaveBeenCalled(); expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
  });
  it("preserves selected files after a database failure and consumes them only after message save", async () => {
    nativeMock.chooseAttachments.mockResolvedValue([file]); nativeMock.createMessage.mockRejectedValueOnce(new Error("database failure"));
    const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
    await user.click(screen.getByRole("button", { name: "Add files" })); await screen.findByRole("button", { name: "Remove brief.txt" });
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Use this brief"); await user.click(screen.getByRole("button", { name: "Send message" })); await screen.findByText("Couldn’t save your message. Your draft is still here.");
    expect(screen.getByRole("button", { name: "Remove brief.txt" })).toBeTruthy(); expect(nativeMock.removeStagedAttachment).not.toHaveBeenCalled();
    nativeMock.messages.mockResolvedValue([{ ...userMessage, attachments: [file] }]); await user.click(screen.getByRole("button", { name: "Send message" })); await screen.findByText("Create this website?");
    expect(nativeMock.createMessage).toHaveBeenLastCalledWith(null, "Use this brief", "auto", [file.id]); expect(screen.queryByRole("button", { name: "Remove brief.txt" })).toBeNull(); expect(screen.getByLabelText("Message attachments").textContent).toContain("brief.txt"); expect(nativeMock.generateWebsite).not.toHaveBeenCalled();
  });
  it("removes staging explicitly and clears unused selected files for a new chat", async () => {
    nativeMock.chooseAttachments.mockResolvedValue([file]); const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
    await user.click(screen.getByRole("button", { name: "Add files" })); await user.click(await screen.findByRole("button", { name: "Remove brief.txt" })); expect(nativeMock.removeStagedAttachment).toHaveBeenCalledWith(file.id);
    await user.click(screen.getByRole("button", { name: "Add files" })); await screen.findByRole("button", { name: "Remove brief.txt" }); await user.click(screen.getByRole("button", { name: "New chat" }));
    expect(screen.queryByRole("button", { name: "Remove brief.txt" })).toBeNull(); expect(nativeMock.removeStagedAttachment).toHaveBeenCalledTimes(2);
  });
  it("discards late native-picker files after switching to a new chat", async () => {
    let finish: (files: typeof file[]) => void = () => {}; nativeMock.chooseAttachments.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" }); await user.click(screen.getByRole("button", { name: "Add files" })); await user.click(screen.getByRole("button", { name: "New chat" }));
    await act(async () => finish([file])); expect(screen.queryByRole("button", { name: "Remove brief.txt" })).toBeNull(); expect(nativeMock.removeStagedAttachment).toHaveBeenCalledWith(file.id);
  });
  it("does not delete attachment bytes while their message save is in flight or overwrite a new draft", async () => {
    nativeMock.chooseAttachments.mockResolvedValue([file]); let finish: (value: typeof conversation) => void = () => {};
    nativeMock.createMessage.mockReturnValue(new Promise((resolve) => { finish = resolve; })); const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
    await user.click(screen.getByRole("button", { name: "Add files" })); await screen.findByRole("button", { name: "Remove brief.txt" }); await user.type(screen.getByRole("textbox", { name: "Message" }), "Old request"); await user.click(screen.getByRole("button", { name: "Send message" }));
    await user.click(screen.getByRole("button", { name: "New chat" })); await user.type(screen.getByRole("textbox", { name: "Message" }), "New draft"); expect(nativeMock.removeStagedAttachment).not.toHaveBeenCalled();
    await act(async () => finish(conversation)); expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toBe("New draft"); expect(nativeMock.removeStagedAttachment).not.toHaveBeenCalled(); expect(screen.queryByRole("button", { name: "Remove brief.txt" })).toBeNull();
  });
});
it("cleans abandoned attachment staging after a delayed message save fails", async () => {
  const file = { id: "abandoned-file", name: "brief.pdf", bytes: 24 }; nativeMock.chooseAttachments.mockResolvedValue([file]);
  let fail: (error: Error) => void = () => {}; nativeMock.createMessage.mockReturnValue(new Promise((_, reject) => { fail = reject; }));
  const user = userEvent.setup(); render(<App />); await screen.findByRole("button", { name: "Output type: Auto" });
  await user.click(screen.getByRole("button", { name: "Add files" })); await screen.findByRole("button", { name: "Remove brief.pdf" }); await user.type(screen.getByRole("textbox", { name: "Message" }), "Old brief"); await user.click(screen.getByRole("button", { name: "Send message" }));
  await user.click(screen.getByRole("button", { name: "New chat" })); await user.type(screen.getByRole("textbox", { name: "Message" }), "Fresh draft"); expect(nativeMock.removeStagedAttachment).not.toHaveBeenCalled();
  await act(async () => fail(new Error("database rejected"))); expect(nativeMock.removeStagedAttachment).toHaveBeenCalledWith(file.id); expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toBe("Fresh draft");
});
