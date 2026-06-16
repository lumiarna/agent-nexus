// Domain types for the Agent Nexus frontend.
//
// Names follow the ADR0002 schema v1 / CONTEXT.md glossary so phase 3 (real IPC)
// can reuse them. Field *shapes* follow prototype/nexus-data.js — the source of
// truth for the mock UI — including UI-derived fields (windows, counts, body).

// ─── Agents / distribution ──────────────────────────────────────────────────

export type AgentName = "Agents" | "Claude Code" | "CodeX" | "Copilot" | "OpenCode";

/** Role of an agent in a Skill/Prompt Agent Matrix row. One source per row. */
export type CellRole = "source" | "target" | "none";
export type Cells = Record<AgentName, CellRole>;

export interface AgentMeta {
  abbr: string;
  color: string;
  configDir: string;
  generic?: boolean;
}

// ─── Provider ───────────────────────────────────────────────────────────────

export type ProviderStatus = "available" | "expired" | "failed" | "nocreds";

export interface ProviderWindow {
  label: string;
  used: number;
  reset: string;
}

export interface Provider {
  id: string;
  name: string;
  plan: string;
  status: ProviderStatus;
  credential: string;
  primary?: number;
  isAgent?: boolean;
  windows?: ProviderWindow[];
  needsParams?: boolean;
  error?: string;
  hiddenCard?: boolean;
}

// ─── Project ────────────────────────────────────────────────────────────────

export type ProjectStatus = "active" | "stale" | "hidden";

export interface Project {
  id: string;
  name: string;
  status: ProjectStatus;
  path: string;
  sessionsDir: string;
  sessionsNote?: string;
  skills: number;
  sessions: number;
  sync: number;
  /** Stable cross-device identity key — always the folder name in the MVP. */
  key: string;
}

export interface GitBaseFolder {
  path: string;
  addedAt: string;
}

export interface ScanResult {
  path: string;
  key: string;
  state: "recorded" | "new";
}

// ─── Skill / Prompt ─────────────────────────────────────────────────────────

export interface Skill {
  id: string;
  name: string;
  scope: "global" | "project";
  projectId?: string;
  desc: string;
  path: string;
  disabled: boolean;
  cells: Cells;
}

export interface Prompt {
  id: string;
  name: string;
  path: string;
  cells: Cells;
}

// ─── Session ────────────────────────────────────────────────────────────────

export type SessionSource = "local" | "cloud" | "both";

export interface Session {
  id: string;
  title: string;
  project: string;
  file: string;
  size: string;
  updated: string;
  source: SessionSource;
  excerpt: string;
  body: string;
}

// ─── Sync ───────────────────────────────────────────────────────────────────

export type TaskDirection = "Distribution" | "Backup" | "Restore/Pull";
export type TaskAction = "symlink" | "copy";
export type TaskStatus = "ok" | "pending" | "never";

export interface Task {
  id: string;
  direction: TaskDirection;
  action: TaskAction;
  source: string;
  targets: string[];
  /** "manual" or a CRON expression. */
  schedule: string;
  lastRun: string;
  status: TaskStatus;
}

export interface TaskGroup {
  id: string;
  name: string;
  tasks: Task[];
}

export type TemplateTask = Pick<
  Task,
  "direction" | "action" | "source" | "targets" | "schedule"
>;

export interface Template {
  id: string;
  name: string;
  desc: string;
  tasks: TemplateTask[];
}

export interface SystemSyncRow {
  asset: string;
  relation: string;
  path: string;
  status: TaskStatus;
}

export interface SystemSync {
  skill: SystemSyncRow[];
  prompt: SystemSyncRow[];
  backup: SystemSyncRow[];
}

// ─── Settings ───────────────────────────────────────────────────────────────

export type TrayMetric = "Used" | "Remaining";

export interface WebdavSettings {
  url: string;
  user: string;
  pass: string;
  status: string;
}

export interface Settings {
  webdav: WebdavSettings;
  trayMetric: TrayMetric;
}

export interface AgentConfigDir {
  envKey: string;
  value: string;
  derived?: boolean;
}

export interface AgentConfigRoot {
  name: AgentName;
  generic: boolean;
  dirs: AgentConfigDir[];
}
