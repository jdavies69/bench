import { Channel, invoke } from "@tauri-apps/api/core";

export type OutputType = "auto" | "chat" | "website" | "application" | "presentation" | "document" | "image" | "agent" | "voice";
export type ExecutionBehavior = "discuss" | "balanced" | "just_do_it";
export type ApprovalBehavior = "always" | "important" | "autonomous";
export type ProviderId = "openrouter" | "openai" | "anthropic" | "xai";
export type WebSearchBackend = "auto" | "brave" | "searxng" | "off";

export interface Project { id: string; name: string; isSystem: boolean; }
export interface Conversation { id: string; projectId: string; title: string; outputType: OutputType; outputSelection: OutputType; updatedAt: string; }
export interface Message { id: string; conversationId: string; role: "user" | "assistant"; content: string; createdAt: string; }
export interface Settings { executionBehavior: ExecutionBehavior; approvalBehavior: ApprovalBehavior; modelProvider: ProviderId; sidebarCollapsed: boolean; webSearchBackend: WebSearchBackend; webSearchUrl: string; }
export interface ProviderStatus { id: ProviderId; label: string; model: string; keySource: "keychain" | "environment" | "none"; }
export interface Snapshot { outputs: OutputDefinition[]; projects: Project[]; conversations: Conversation[]; settings: Settings; providers: ProviderStatus[]; webSearchKeySource: "keychain" | "environment" | "none"; webSearchStatus: "openrouter" | "unavailable" | "off"; }
export interface StreamEvent { kind: "delta" | "done" | "tool"; text: string; }
export interface ToolActivity { id: string; userMessageId: string | null; toolName: string; status: string; summary: string; }
export interface OutputDefinition { id: Exclude<OutputType, "auto">; label: string; implemented: boolean; workspace: "conversation" | "canvas"; }
export interface ActionReview { id: string; title: string; detail: string; conversationId: string; }
export interface WebsiteRevision { revision: number; current: boolean; }
export interface WebsitePage { path: string; html: string; }
export interface WebsiteState { pages: WebsitePage[]; css: string; revision: number; requestCount: number; }

export const native = {
  snapshot: () => invoke<Snapshot>("load_snapshot"),
  messages: (conversationId: string) => invoke<Message[]>("load_messages", { conversationId }),
  toolActivity: (conversationId: string) => invoke<ToolActivity[]>("load_tool_activity", { conversationId }),
  createMessage: (conversationId: string | null, content: string, outputType: OutputType) =>
    invoke<Conversation>("create_user_message", { conversationId, content, outputType }),
  stream: (conversationId: string, onEvent: (event: StreamEvent) => void) => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("stream_response", { conversationId, onEvent: channel });
  },
  createProject: (name: string) => invoke<Project>("create_project", { name }),
  move: (conversationId: string, projectId: string) => invoke<void>("move_conversation", { conversationId, projectId }),
  settings: (executionBehavior: ExecutionBehavior, approvalBehavior: ApprovalBehavior) =>
    invoke<Settings>("update_settings", { executionBehavior, approvalBehavior }),
  setSidebarCollapsed: (collapsed: boolean) => invoke<Settings>("set_sidebar_collapsed", { collapsed }),
  saveProviderKey: (provider: ProviderId, key: string) => invoke<void>("save_provider_key", { provider, key }),
  openOpenRouterSetup: () => invoke<void>("open_openrouter_setup"),
  openOpenRouterUsage: () => invoke<void>("open_openrouter_usage"),
  openOpenRouterBilling: () => invoke<void>("open_openrouter_billing"),
  removeProviderKey: (provider: ProviderId) => invoke<void>("remove_provider_key", { provider }),
  selectProvider: (provider: ProviderId) => invoke<Settings>("select_provider", { provider }),
  updateModel: (provider: ProviderId, model: string) => invoke<void>("update_model", { provider, model }),
  saveWebSearchKey: (key: string) => invoke<Settings>("save_web_search_key", { key }),
  removeWebSearchKey: () => invoke<Settings>("remove_web_search_key"),
  configureWebSearch: (backend: WebSearchBackend, searxngUrl: string) => invoke<Settings>("configure_web_search", { backend, searxngUrl }),
  search: (query: string) => invoke<Conversation[]>("search_conversations", { query }),
  loadWebsite: (conversationId: string) => invoke<WebsiteState | null>("load_website", { conversationId }),
  generateWebsite: (conversationId: string, approvalToken?: string) => invoke<WebsiteState>("generate_website", { conversationId, approvalToken }),
  websiteRevisions: (conversationId: string) => invoke<WebsiteRevision[]>("list_website_revisions", { conversationId }),
  restoreWebsite: (conversationId: string, revision: number, approvalToken?: string) => invoke<WebsiteState>("restore_website_revision", { conversationId, revision, approvalToken }),
  prepareWebsiteAction: (conversationId: string, operation: "generate" | "restore", targetRevision?: number) => invoke<ActionReview | null>("prepare_website_action", { conversationId, operation, targetRevision }),
  approveAction: (reviewId: string) => invoke<string>("approve_action", { reviewId }),
};
