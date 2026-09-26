import { Channel, invoke } from "@tauri-apps/api/core";

/** `agent` runs the user's own coding agent; the rest go through the panel's loop. */
export type ApiKind = "agent" | "open-ai-compatible";
export type Effort = string;

export interface AgentStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
  /** null when the CLI could not say. */
  signed_in: boolean | null;
  account: string | null;
  error: string | null;
}

export interface ProviderStatus {
  id: string;
  label: string;
  kind: ApiKind;
  base_url: string | null;
  default_base: string | null;
  agent: AgentStatus | null;
  /** The command that signs the CLI in, run by the user in a terminal. */
  login: string | null;
  install: string | null;
  /** Bytes the app downloads itself; such an agent also signs in from the app. */
  download: number | null;
  /** The vendor's install command, run in a terminal from the settings page. */
  install_command: string | null;
  ready: boolean;
}

export type InstallProgress = { kind: "download"; done: number; total: number } | { kind: "unpack" };

export interface ModelInfo {
  id: string;
  label: string;
  /** Effort levels the model takes, lowest first; empty hides the selector. */
  efforts: Effort[];
  /** Current generation or one back — the UI groups these first. */
  recommended: boolean;
  /** Offers a faster service tier. */
  fast: boolean;
}

export const listProviders = (refresh = false, only?: string) => invoke<ProviderStatus[]>("ai_providers", { refresh, only });
export const listModels = (provider: string) => invoke<ModelInfo[]>("ai_models", { provider });
export const setKey = (provider: string, key: string) => invoke<void>("ai_key_set", { provider, key });
export const clearKey = (provider: string) => invoke<void>("ai_key_clear", { provider });
export const setBase = (provider: string, baseUrl: string) =>
  invoke<void>("ai_base_set", { provider, baseUrl });
export const setAgentPath = (provider: string, path: string) =>
  invoke<void>("agent_path_set", { provider, path });

export function installAgent(provider: string, onProgress: (p: InstallProgress) => void) {
  const channel = new Channel<InstallProgress>();
  channel.onmessage = onProgress;
  return invoke<void>("agent_install", { provider, onProgress: channel });
}

export const signInAgent = (provider: string) => invoke<void>("agent_sign_in", { provider });
export const cancelSignIn = () => invoke<void>("agent_sign_in_cancel");
export const signOutAgent = (provider: string) => invoke<void>("agent_sign_out", { provider });
export const uninstallAgent = (provider: string) => invoke<void>("agent_uninstall", { provider });
/** Open a terminal running the install or sign-in command; returns the command it ran. */
export const openTerminal = (provider: string, action: "install" | "login") => invoke<string>("agent_terminal", { provider, action });

/** The saved level if the model takes it, else the nearest sensible one. */
export function clampEffort(efforts: Effort[], effort: Effort): Effort | null {
  if (!efforts.length) return null;
  if (efforts.includes(effort)) return effort;
  return efforts.includes("medium") ? "medium" : efforts[efforts.length - 1];
}
