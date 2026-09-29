import type { ComponentType } from "react";
import type { OutputType, WebsiteRevision, WebsiteState } from "../native";
import { WebsiteWorkspace } from "./WebsiteWorkspace";

export interface OutputWorkspaceProps {
  state: WebsiteState; revisions: WebsiteRevision[]; busy: boolean; onRestore: (revision: number) => void;
}

// New output modules mount here; the shell and provider gateway stay unchanged.
export const workspaceModules: Partial<Record<OutputType, ComponentType<OutputWorkspaceProps>>> = {
  website: WebsiteWorkspace,
};
