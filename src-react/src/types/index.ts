// Domain types for the Agent Nexus frontend.
//
// Names follow the ADR0002 schema v1 / CONTEXT.md glossary and the desktop IPC
// payloads exposed by the Tauri backend. Some fields are UI-derived summaries
// such as provider quota windows, project counts, and session preview bodies.

// ─── Agents / distribution ──────────────────────────────────────────────────

import type { AgentName as CapabilityAgentName } from "../config/agents.js";

export type AgentName = CapabilityAgentName;

/** Agent canonical matrices have exactly one source; Project custom
 * placement matrices deliberately cannot represent a source cell. */
export type CellRole = "source" | "target" | "none";
export type AgentCellRole = CellRole;
export type PlacementCellRole = "target" | "none";
export type Cells = Record<AgentName, CellRole>;
export type AgentCells = Record<AgentName, AgentCellRole>;
export type PlacementCells = Record<AgentName, PlacementCellRole>;

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
  prompts: number;
  sessions: number;
  sync: number;
  /** Stable cross-device identity key — always the folder name in the MVP. */
  key: string;
  /** Extra Project custom skills directories scanned alongside the fixed Agent
   *  project skills dirs. Relative paths resolve against the Project root; absolute
   *  paths may live outside the repo. Absent on backends that predate the feature. */
  customSkillsDirs?: string[];
  /** Extra Prompt files explicitly registered for this Project, scanned alongside
   *  the primary AGENTS.md / CLAUDE.md. Each entry is a path relative to the Project
   *  root whose filename must match an Agent `projectPromptFile` glob (AGENTS*.md /
   *  CLAUDE*.md). Absent on backends that predate the feature. */
  extraPromptFiles?: string[];
}

/** Global defaults a brand-new Project inherits at creation. A snapshot applied
 *  once in `record_project`; editing them never retro-applies to existing projects,
 *  which keep their own per-Project overrides. */
export interface ProjectDefaults {
  customSkillsDirs: string[];
  extraPromptFiles: string[];
  sessionsDir: string;
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

export interface ProjectRef {
  id: string;
  name: string;
}

export interface SkillSummary {
  /** Canonical backend `skills.id` for every row variant. */
  skillId: string;
  name: string;
  desc: string;
  path: string;
  disabled: boolean;
}

export type SkillContext =
  | { kind: "global" }
  | { kind: "project"; project: ProjectRef };

export type ProjectCustomDestinationState =
  | { kind: "global"; cells: PlacementCells }
  | { kind: "project"; project: ProjectRef; cells: PlacementCells };

export interface AgentCanonicalSkillRow {
  kind: "agentCanonical";
  rowKey: string;
  skill: SkillSummary;
  context: SkillContext;
  sourceAgent: AgentName;
  cells: AgentCells;
}

export interface ProjectCustomCanonicalSkillRow {
  kind: "projectCustomCanonical";
  rowKey: string;
  skill: SkillSummary;
  sourceProject: ProjectRef;
  destinations: ProjectCustomDestinationState[];
}

export interface ProjectCustomIncomingSkillRow {
  kind: "projectCustomIncoming";
  rowKey: string;
  skill: SkillSummary;
  sourceProject: ProjectRef;
  targetProject: ProjectRef;
  cells: PlacementCells;
}

/** Read-side Skill row. The discriminant makes canonical Agent identity,
 * Project Custom Source identity, and incoming Placement identity exclusive. */
export type Skill =
  | AgentCanonicalSkillRow
  | ProjectCustomCanonicalSkillRow
  | ProjectCustomIncomingSkillRow;

export type ProjectCustomSkillDestination =
  | { kind: "global" }
  | { kind: "project"; projectId: string };

export type ProjectCustomSkillIntent =
  | {
      kind: "setTargetEnabled";
      skillId: string;
      destination: ProjectCustomSkillDestination;
      enabled: boolean;
    }
  | {
      kind: "setAgentPlacement";
      skillId: string;
      destination: ProjectCustomSkillDestination;
      agent: AgentName;
      enabled: boolean;
    };

export interface ProjectCustomSkillMutationResult {
  changed: boolean;
  skills: Skill[];
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
  collapsed: boolean;
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
  /** Provider ids shown as a Windows-taskbar tray icon. */
  trayVisibility: string[];
}
