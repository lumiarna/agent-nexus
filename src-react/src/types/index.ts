// Domain types for the Agent Nexus frontend.
//
// Names follow the ADR0002 schema v1 / CONTEXT.md glossary and the desktop IPC
// payloads exposed by the Tauri backend. Some fields are UI-derived summaries
// such as provider quota windows, project counts, and session preview bodies.

// ─── Agents / distribution ──────────────────────────────────────────────────

import type { AgentName as CapabilityAgentName } from "../config/agents.js";

export type AgentName = CapabilityAgentName;

/** Role of an agent in a Skill/Prompt Agent Matrix row. One source per row. */
export type CellRole = "source" | "target" | "none";
export type Cells = Record<AgentName, CellRole>;

// ─── Provider ───────────────────────────────────────────────────────────────

export type ProviderStatus = "available" | "expired" | "failed" | "nocreds";

export interface ProviderWindow {
  label: string;
  used: number;
  valueLabel?: string;
  valueOnly?: boolean;
  reset: string;
  kind?: "rolling" | "weekly" | "monthly";
  resetAt?: string;
  unlimited?: boolean;
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
  /** Extra Project custom skills directories scanned alongside the fixed Agent
   *  project skills dirs. Relative paths resolve against the Project root; absolute
   *  paths may live outside the repo. Absent on backends that predate the feature. */
  customSkillsDirs?: string[];
}

export interface GitBaseFolder {
  id: string;
  path: string;
  addedAt: string;
}

export interface ScanResult {
  path: string;
  key: string;
  state: "recorded" | "new";
}

// ─── Skill / Prompt ─────────────────────────────────────────────────────────

/** Canonical source kind for a Skill.
 *  `agent` — owned by a fixed Agent project/global skills dir; the Agent Matrix
 *  has exactly one `source` cell.
 *  `project_custom` — discovered from a Project `custom_skills_dir`; the canonical
 *  source belongs to no Agent, so the row has no `source` cell. Global placements
 *  are managed symlinks/junctions and show only as `target` / `none`. */
export type SkillSourceKind = "agent" | "project_custom";

export interface Skill {
  id: string;
  name: string;
  scope: "global" | "project";
  projectId?: string;
  desc: string;
  path: string;
  disabled: boolean;
  cells: Cells;
  /** Canonical source kind. Absent payloads are treated as `agent` for
   *  backward compatibility with backends that predate project custom sources. */
  sourceKind?: SkillSourceKind;
  /** Owning Agent when `sourceKind === "agent"`; `undefined` for `project_custom`. */
  sourceAgent?: AgentName;
}

export interface Prompt {
  id: string;
  name: string;
  scope: "global" | "project";
  projectId?: string;
  path: string;
  /** File body, read from the canonical UTF-8 source for client-side search. */
  content: string;
  cells: Cells;
}

// ─── Session ────────────────────────────────────────────────────────────────

export type SessionSource = "local" | "cloud" | "both";

export interface Session {
  id: string;
  title: string;
  projectName?: string;
  project: string;
  file: string;
  size: string;
  updated: string;
  source: SessionSource;
  excerpt: string;
  body: string;
}

// ─── Sync ───────────────────────────────────────────────────────────────────

export type TaskDirection = "Distribution" | "Push" | "Pull";
export type TaskAction = "Symlink" | "Junction" | "Copy";
export type LocationType = "Local" | "Cloud";
export type TaskStatus = "ok" | "pending" | "failed" | "never" | "skipped";
export type TaskLinkState = "present" | "missing";

export interface Task {
  id: string;
  direction: TaskDirection;
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targetType: LocationType;
  target: string;
  /** "manual" or a schedule expression. */
  schedule: string;
  /** Epoch seconds of the last run, or `null` when never run. Formatted in local time by the UI. */
  lastRunAt: number | null;
  status: TaskStatus;
  /** Placement health for link actions (Symlink/Junction + Local target).
   *  `missing` means the symlink/junction was removed out-of-band; the task row
   *  stays so the user can see the relationship is broken. Copy and Cloud targets
   *  always report `present` — they own no link placement. */
  linkState: TaskLinkState;
}

export interface TaskGroup {
  id: string;
  name: string;
  tasks: Task[];
}

export interface SessionBackup {
  projectKey: string;
  task: Task;
}

export type TemplateTask = Pick<
  Task,
  "action" | "sourceType" | "source" | "targetType" | "target" | "schedule"
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

export type ProjectSymlinkStatus = "ok" | "missing";
export type ProjectSymlinkKind = "directory" | "file" | "other" | "missing";

export interface ProjectSymlink {
  id: string;
  sourcePath: string;
  sourceProjectId?: string;
  sourceProjectName?: string;
  targetPath: string;
  targetProjectId?: string;
  targetProjectName?: string;
  /** How the placement is realized on disk — Symlink (Unix/elevated Windows) or Junction (Windows). */
  linkType: "Symlink" | "Junction";
  linkKind: ProjectSymlinkKind;
  status: ProjectSymlinkStatus;
}

// ─── Settings ───────────────────────────────────────────────────────────────

export type TrayMetric = "Used" | "Remaining";

export interface WebdavSettings {
  url: string;
  user: string;
  pass: string;
  remoteRoot: string;
  status?: string;
}

export interface Settings {
  webdav: WebdavSettings;
  trayMetric: TrayMetric;
}
