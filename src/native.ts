import { Channel, invoke } from "@tauri-apps/api/core";

export type OutputType = "auto" | "chat" | "website" | "application" | "presentation" | "document" | "image" | "agent" | "voice";
export type ExecutionBehavior = "discuss" | "balanced" | "just_do_it";
export type ApprovalBehavior = "always" | "important" | "autonomous";
export type ProviderId = "openrouter" | "openai" | "anthropic" | "xai";
export type WebSearchBackend = "auto" | "brave" | "searxng" | "off";

export interface Project { id: string; name: string; isSystem: boolean; }
export interface Conversation { id: string; projectId: string; title: string; outputType: OutputType; outputSelection: OutputType; updatedAt: string; }
export interface AttachmentMetadata { id: string; name: string; bytes: number; }
export interface Message { id: string; conversationId: string; role: "user" | "assistant"; content: string; createdAt: string; attachments?: AttachmentMetadata[]; }
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
export interface OpenRouterUsage { usageDaily: number; usageWeekly: number; usageMonthly: number; usageTotal: number; limit: number | null; limitRemaining: number | null; limitReset: string | null; byokUsageMonthly: number | null; }

export type TextArtifactContent = { title: string; markdown: string } | { title: string; slides: { title: string; body: string; notes: string }[] };
export type ApplicationExpression = { op: "constant"; value: number } | { op: "input"; id: string } | { op: "add" | "subtract" | "multiply" | "divide" | "min" | "max"; left: ApplicationExpression; right: ApplicationExpression };
export interface ApplicationContent { title: string; description: string; fields: { id: string; label: string; type: "number" | "text" | "select" | "toggle"; default: number | string | boolean; options?: string[] | null }[]; outputs: { label: string; expression: ApplicationExpression }[]; }
export type ApplicationValues = Record<string, number | string | boolean>;
export interface MediaContent { mimeType: string; dataBase64: string; model: string; generationId: string | null; prompt: string; voice: string | null; }
type ArtifactBase = { version: number; revision: number; requestCount: number };
export type ArtifactState = ArtifactBase & (
  { kind: "document"; content: { title: string; markdown: string } } |
  { kind: "presentation"; content: { title: string; slides: { title: string; body: string; notes: string }[] } } |
  { kind: "application"; content: ApplicationContent } |
  { kind: "agent"; content: { title: string; markdown: string; steps: { tool: "read_conversation" | "draft_report" | "inspect_draft"; summary: string }[] } } |
  { kind: "image" | "voice"; content: MediaContent }
);
export type WorkspaceState = (WebsiteState & { kind: "website" }) | ArtifactState;
export type ArtifactExportFormat = "text" | "html" | "binary";

export const native = {
  snapshot: () => invoke<Snapshot>("load_snapshot"),
  messages: (conversationId: string) => invoke<Message[]>("load_messages", { conversationId }),
  toolActivity: (conversationId: string) => invoke<ToolActivity[]>("load_tool_activity", { conversationId }),
  chooseAttachments: () => invoke<AttachmentMetadata[]>("choose_attachments"),
  removeStagedAttachment: (attachmentId: string) => invoke<void>("remove_staged_attachment", { attachmentId }),
  createMessage: (conversationId: string | null, content: string, outputType: OutputType, attachmentIds: string[] = []) =>
    invoke<Conversation>("create_user_message", { conversationId, content, outputType, attachmentIds }),
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
  connectOpenRouter: () => invoke<void>("connect_openrouter"),
  cancelOpenRouterConnect: () => invoke<void>("cancel_openrouter_connect"),
  openOpenRouterSetup: () => invoke<void>("open_openrouter_setup"),
  openOpenRouterUsage: () => invoke<void>("open_openrouter_usage"),
  openOpenRouterBilling: () => invoke<void>("open_openrouter_billing"),
  openRouterUsage: () => invoke<OpenRouterUsage>("load_openrouter_usage"),
  removeProviderKey: (provider: ProviderId) => invoke<void>("remove_provider_key", { provider }),
  selectProvider: (provider: ProviderId) => invoke<Settings>("select_provider", { provider }),
  updateModel: (provider: ProviderId, model: string) => invoke<void>("update_model", { provider, model }),
  saveWebSearchKey: (key: string) => invoke<Settings>("save_web_search_key", { key }),
  removeWebSearchKey: () => invoke<Settings>("remove_web_search_key"),
  configureWebSearch: (backend: WebSearchBackend, searxngUrl: string) => invoke<Settings>("configure_web_search", { backend, searxngUrl }),
  search: (query: string) => invoke<Conversation[]>("search_conversations", { query }),
  loadApplicationValues: (conversationId: string) => invoke<ApplicationValues>("load_application_values", { conversationId }),
  saveApplicationValues: (conversationId: string, revision: number, values: ApplicationValues) => invoke<ApplicationValues>("save_application_values", { conversationId, revision, values }),
  loadArtifact: (conversationId: string) => invoke<ArtifactState | null>("load_artifact", { conversationId }),
  generateArtifact: (conversationId: string, approvalToken?: string) => invoke<ArtifactState>("generate_artifact", { conversationId, approvalToken }),
  generateAgentOutput: (conversationId: string, approvalToken?: string) => invoke<ArtifactState>("generate_agent_output", { conversationId, approvalToken }),
  generateMediaOutput: (conversationId: string, approvalToken?: string) => invoke<ArtifactState>("generate_media_output", { conversationId, approvalToken }),
  artifactRevisions: (conversationId: string) => invoke<WebsiteRevision[]>("list_artifact_revisions", { conversationId }),
  restoreArtifact: (conversationId: string, revision: number, approvalToken?: string) => invoke<ArtifactState>("restore_artifact_revision", { conversationId, revision, approvalToken }),
  saveArtifactEdits: (conversationId: string, content: TextArtifactContent, approvalToken?: string) => invoke<ArtifactState>("save_artifact_edits", { conversationId, content, approvalToken }),
  prepareArtifactAction: (conversationId: string, operation: "generate" | "restore" | "edit", targetRevision?: number, content?: TextArtifactContent) => invoke<ActionReview | null>("prepare_artifact_action", { conversationId, operation, targetRevision, content }),
  exportArtifact: (conversationId: string, format: ArtifactExportFormat) => invoke<boolean>("export_artifact", { conversationId, format }),
  loadWebsite: (conversationId: string) => invoke<WebsiteState | null>("load_website", { conversationId }),
  generateWebsite: (conversationId: string, approvalToken?: string) => invoke<WebsiteState>("generate_website", { conversationId, approvalToken }),
  websiteRevisions: (conversationId: string) => invoke<WebsiteRevision[]>("list_website_revisions", { conversationId }),
  restoreWebsite: (conversationId: string, revision: number, approvalToken?: string) => invoke<WebsiteState>("restore_website_revision", { conversationId, revision, approvalToken }),
  prepareWebsiteAction: (conversationId: string, operation: "generate" | "restore", targetRevision?: number) => invoke<ActionReview | null>("prepare_website_action", { conversationId, operation, targetRevision }),
  approveAction: (reviewId: string) => invoke<string>("approve_action", { reviewId }),
};
