import type { ComponentType } from "react";
import type { ArtifactExportFormat, OutputType, TextArtifactContent, WebsiteRevision, WorkspaceState } from "../native";
import { WebsiteWorkspace } from "./WebsiteWorkspace";
import { DocumentWorkspace } from "./DocumentWorkspace";
import { PresentationWorkspace } from "./PresentationWorkspace";
import { AgentWorkspace } from "./AgentWorkspace";
import { ApplicationWorkspace } from "./ApplicationWorkspace";
import { MediaWorkspace } from "./MediaWorkspace";
export interface OutputWorkspaceProps {
  conversationId: string; state: WorkspaceState; revisions: WebsiteRevision[]; busy: boolean; onRestore: (revision: number) => void;
  onExport: (format: ArtifactExportFormat) => void; onEdit: (content: TextArtifactContent) => void;
}
export const workspaceModules: Partial<Record<OutputType, ComponentType<OutputWorkspaceProps>>> = {
  agent: ({ state, busy, onExport }) => state.kind === "agent" ? <AgentWorkspace state={state} busy={busy} onExport={() => onExport("text")} /> : null,
  application: ({ state, conversationId, busy, onExport }) => state.kind === "application" ? <ApplicationWorkspace key={`${conversationId}:${state.revision}`} conversationId={conversationId} state={state} busy={busy} onExport={() => onExport("html")} /> : null,
  website: ({ state, ...props }) => state.kind === "website" ? <WebsiteWorkspace state={state} {...props} /> : null,
  document: ({ state, busy, onExport, onEdit }) => state.kind === "document" ? <DocumentWorkspace state={state} busy={busy} onExport={() => onExport("text")} onEdit={onEdit} /> : null,
  presentation: ({ state, busy, onExport, onEdit }) => state.kind === "presentation" ? <PresentationWorkspace state={state} busy={busy} onExport={() => onExport("text")} onEdit={onEdit} /> : null,
  image: ({ state, busy, onExport }) => state.kind === "image" ? <MediaWorkspace state={state} busy={busy} onExport={() => onExport("binary")} /> : null,
  voice: ({ state, busy, onExport }) => state.kind === "voice" ? <MediaWorkspace state={state} busy={busy} onExport={() => onExport("binary")} /> : null,
};
