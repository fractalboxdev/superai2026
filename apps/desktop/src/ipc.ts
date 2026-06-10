// Typed surface over the Tauri commands exposed by src-tauri (spec 10).
// The app is a thin shell: every call here either launches/observes the
// bundled `sync` binary or touches app-local settings — never brain data.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Role = "host" | "member";

export type SupervisorStatus = "starting" | "healthy" | "degraded" | "stopped";

export interface AppSettings {
  configured: boolean;
  role: Role;
  principal: string;
  /** Host: bind address for `serve`. Member: the host's relay address. */
  relayAddr: string;
  /** Member only: room/document id to sync. */
  doc: string;
  /** Brain home; null = default `~/.contextful`. Host only. */
  brainHome: string | null;
  /** `stub` (offline) · `bedrock` (cloud) · `lmstudio` (on-prem). */
  inference: string;
  autostart: boolean;
  webAppUrl: string;
  updateChannel: string;
}

export interface SupervisorSnapshot {
  status: SupervisorStatus;
  /** Human-readable detail, e.g. "relay reachable" or "tailnet offline". */
  detail: string;
  pid: number | null;
  restarts: number;
}

export interface TailscaleInfo {
  installed: boolean;
  running: boolean;
  /** MagicDNS name of this device, e.g. `studio.tail1234.ts.net`. */
  dnsName: string | null;
  /** Derived sync WS url members point at (host role). */
  syncUrl: string | null;
}

export interface AppState {
  settings: AppSettings;
  supervisor: SupervisorSnapshot;
  tailscale: TailscaleInfo;
  /** Seeded principals the wizard offers (spec 03/07 demo control plane). */
  knownPrincipals: string[];
  /** Resolved path of the bundled sync binary (diagnostics). */
  sidecarPath: string | null;
  launchAgentInstalled: boolean;
}

export interface IdentityInfo {
  principal: string;
  /** Where the private key lives — always the Keychain, never a file. */
  keychainService: string;
  created: boolean;
}

export const getAppState = () => invoke<AppState>("get_app_state");

export const saveSettings = (patch: Partial<AppSettings>) =>
  invoke<AppSettings>("save_settings", { patch });

export const ensureIdentity = (principal: string, role: Role) =>
  invoke<IdentityInfo>("ensure_identity", { principal, role });

export const detectTailscale = () => invoke<TailscaleInfo>("detect_tailscale");

export const startService = () => invoke<void>("start_service");
export const stopService = () => invoke<void>("stop_service");
export const restartService = () => invoke<void>("restart_service");

export const getLogs = (limit?: number) =>
  invoke<string[]>("get_logs", { limit });

export const setAutostart = (enable: boolean) =>
  invoke<boolean>("set_autostart", { enable });

export const markConfigured = () => invoke<AppSettings>("mark_configured");

export const openWebApp = () => invoke<void>("open_web_app");
export const revealBrain = () => invoke<void>("reveal_brain");
export const copySyncUrl = () => invoke<string>("copy_sync_url");

const on =
  <T,>(event: string) =>
  (cb: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(event, (e) => cb(e.payload));

export const onSupervisorStatus = on<SupervisorSnapshot>("supervisor:status");
export const onSupervisorLog = on<string>("supervisor:log");
export const onNavigate = on<string>("navigate");
