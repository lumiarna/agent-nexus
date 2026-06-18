// In-memory mock data — typed port of prototype/nexus-data.js.
//
// Read accessors return deep clones so pages can hold mutable copies in local
// state without touching the seed (same read/write boundary as the prototype).
// Phase 3 replaces this module with lib/api/* (typed invoke) + React Query.

import type {
  GitBaseFolder,
  Project,
  Prompt,
  Provider,
  ScanResult,
  Session,
  SessionSource,
  Settings,
  Skill,
  SystemSync,
  TaskGroup,
  Template,
} from "@/types";

const clone = <T>(x: T): T => structuredClone(x);

// ─── Providers ──────────────────────────────────────────────────────────────

const PROVIDERS: Provider[] = [
  {
    id: "claude", name: "Claude Code", plan: "Max 20×", status: "available",
    credential: "~/.claude", primary: 68, isAgent: true,
    windows: [
      { label: "5-hour limit", used: 42, reset: "Resets in 2h 14m" },
      { label: "Weekly limit", used: 68, reset: "Resets Mon 09:00" },
    ],
  },
  {
    id: "codex", name: "CodeX", plan: "ChatGPT Plus", status: "available",
    credential: "~/.codex/auth.json", primary: 51, isAgent: true,
    windows: [
      { label: "5-hour limit", used: 23, reset: "Resets in 3h 40m" },
      { label: "Weekly limit", used: 51, reset: "Resets Thu 14:00" },
    ],
  },
  {
    id: "copilot", name: "Copilot", plan: "Business", status: "available",
    credential: "$GITHUB_TOKEN", primary: 77, isAgent: true,
    windows: [{ label: "Premium requests (monthly)", used: 77, reset: "Resets Jul 1" }],
  },
  {
    id: "opencode-go", name: "OpenCode Go", plan: "Workspace", status: "expired",
    credential: "manual · workspace id + cookie", needsParams: true,
  },
  {
    id: "minimax", name: "MiniMax CN", plan: "Pay-as-you-go", status: "available",
    credential: "~/.local/share/opencode/auth.json", primary: 12,
    windows: [{ label: "Daily tokens", used: 12, reset: "Resets 00:00 CST" }],
  },
  {
    id: "deepseek", name: "DeepSeek", plan: "—", status: "failed",
    credential: "~/.local/share/opencode/auth.json",
    error: "HTTP 503 from quota endpoint. Last success 41m ago.",
  },
  {
    id: "openrouter", name: "OpenRouter", plan: "Credits", status: "available",
    credential: "~/.local/share/opencode/auth.json", primary: 34,
    windows: [{ label: "Credit balance", used: 34, reset: "$66.00 of $100.00 left" }],
  },
  {
    id: "minimax-token", name: "MiniMax Token Plan CN", plan: "Token plan",
    status: "nocreds", credential: "not found", hiddenCard: true,
  },
];

// ─── Projects (Project Key == folder name, no edit) ─────────────────────────

const PROJECTS: Project[] = [
  { id: "oll-context", name: "oll-context", status: "active", path: "D:/Workspace/oll-context", sessionsDir: "__sessions", skills: 2, sessions: 18, sync: 6, key: "oll-context" },
  { id: "tap", name: "tap", status: "active", path: "D:/Workspace/tap", sessionsDir: "__sessions", skills: 1, sessions: 7, sync: 3, key: "tap" },
  { id: "tap-kit", name: "tap-kit", status: "active", path: "D:/Workspace/tap-kit", sessionsDir: "__sessions", skills: 0, sessions: 3, sync: 2, key: "tap-kit" },
  { id: "awesome-vibe-coding", name: "awesome-vibe-coding", status: "active", path: "D:/Workspace/awesome-vibe-coding", sessionsDir: "docs/__sessions", sessionsNote: " · override", skills: 1, sessions: 5, sync: 2, key: "awesome-vibe-coding" },
  { id: "agent-nexus", name: "agent-nexus", status: "active", path: "D:/Workspace/agent-nexus", sessionsDir: "__sessions", skills: 3, sessions: 12, sync: 4, key: "agent-nexus" },
  { id: "legacy-cli", name: "legacy-cli", status: "stale", path: "E:/Old/legacy-cli", sessionsDir: "__sessions", skills: 1, sessions: 9, sync: 2, key: "legacy-cli" },
  { id: "sandbox", name: "sandbox", status: "hidden", path: "D:/Workspace/sandbox", sessionsDir: "__sessions", skills: 0, sessions: 2, sync: 0, key: "sandbox" },
];

const GIT_BASE_FOLDERS: GitBaseFolder[] = [
  { id: "workspace", path: "D:/Workspace", addedAt: "2026-06-10" },
];
const SCAN_BASE = GIT_BASE_FOLDERS[0].path;
const SCAN_RESULTS: ScanResult[] = [
  { path: "D:/Workspace/oll-context", key: "oll-context", state: "recorded" },
  { path: "D:/Workspace/tap", key: "tap", state: "recorded" },
  { path: "D:/Workspace/new-service", key: "new-service", state: "new" },
  { path: "D:/Workspace/billing-api", key: "billing-api", state: "new" },
  { path: "D:/Workspace/docs-site", key: "docs-site", state: "new" },
];

// ─── Skills (cells keyed by agent name; one source per row) ─────────────────

const SKILLS: Skill[] = [
  { id: "sk1", name: "tap-builder", scope: "global", desc: "Scaffold TAP modules from a spec file", path: "~/.claude/skills/tap-builder", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "source", CodeX: "target", Copilot: "target", OpenCode: "none" } },
  { id: "sk2", name: "commit-helper", scope: "global", desc: "Generate conventional commit messages", path: "~/.claude/skills/commit-helper", disabled: false, cells: { "Generic Agent": "target", "Claude Code": "source", CodeX: "target", Copilot: "target", OpenCode: "target" } },
  { id: "sk3", name: "issue-triage", scope: "global", desc: "Apply canonical triage labels to issues", path: "~/.config/opencode/skills/issue-triage", disabled: true, cells: { "Generic Agent": "none", "Claude Code": "none", CodeX: "none", Copilot: "none", OpenCode: "source" } },
  { id: "sk4", name: "pdf-extractor", scope: "global", desc: "Extract structured data from PDFs", path: "~/.codex/skills/pdf-extractor", disabled: false, cells: { "Generic Agent": "target", "Claude Code": "none", CodeX: "source", Copilot: "none", OpenCode: "target" } },
  { id: "sk5", name: "domain-doctor", scope: "global", desc: "Maintain CONTEXT.md domain docs", path: "~/.claude/skills/domain-doctor", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "source", CodeX: "none", Copilot: "target", OpenCode: "none" } },
  { id: "sk6", name: "tap-builder", scope: "project", projectId: "oll-context", desc: "Project-scoped TAP scaffolder", path: ".github/skills/tap-builder", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "source", CodeX: "target", Copilot: "target", OpenCode: "none" } },
  { id: "sk7", name: "test-runner", scope: "project", projectId: "oll-context", desc: "Run and summarize the test suite", path: ".codex/skills/test-runner", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "none", CodeX: "source", Copilot: "none", OpenCode: "none" } },
  { id: "sk8", name: "release-notes", scope: "project", projectId: "tap", desc: "Draft release notes from merged PRs", path: ".claude/skills/release-notes", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "source", CodeX: "none", Copilot: "target", OpenCode: "none" } },
  { id: "sk9", name: "vibe-lint", scope: "project", projectId: "awesome-vibe-coding", desc: "Lint vibe-coding examples", path: ".agents/skills/vibe-lint", disabled: false, cells: { "Generic Agent": "target", "Claude Code": "none", CodeX: "none", Copilot: "none", OpenCode: "source" } },
  { id: "sk10", name: "context-doctor", scope: "project", projectId: "agent-nexus", desc: "Maintain CONTEXT.md domain docs", path: ".claude/skills/context-doctor", disabled: false, cells: { "Generic Agent": "none", "Claude Code": "source", CodeX: "target", Copilot: "none", OpenCode: "none" } },
];

// ─── Prompts (global single-file assets) ────────────────────────────────────

const PROMPTS: Prompt[] = [
  { id: "pr1", name: "Global Instructions", path: "~/.claude/CLAUDE.md", cells: { "Generic Agent": "target", "Claude Code": "source", CodeX: "target", Copilot: "target", OpenCode: "target" } },
  { id: "pr2", name: "Agent Conventions", path: "~/.codex/AGENTS.md", cells: { "Generic Agent": "target", "Claude Code": "none", CodeX: "source", Copilot: "none", OpenCode: "target" } },
];

// ─── Sessions ───────────────────────────────────────────────────────────────

const SESSIONS: Session[] = [
  { id: "s1", title: "260615-1059-Agent Nexus需求审视", project: "agent-nexus", file: "__sessions/260615-1059-Agent Nexus需求审视.md", size: "11.3 KB", updated: "2026-06-15 13:19", source: "both", excerpt: "审视基于当前项目演化出的新项目需求边界、实体模型与范围优先级。", body: "# 主题一\n\n## 设计决策\n- 先不讨论实现，优先澄清产品定位、范围边界、实体关系与同步语义。\n- 一级导航命名最终确认：单数形式 Provider / Project / Skill / Prompt / Session / Sync。\n- Sync 语义确认：只做单向，仅支持 Backup 与 Distribution 两种单向模式。\n- Agent Matrix 作为高层快捷控制器，用户在表格内直接切换关系。" },
  { id: "s2", title: "260615-1958-原型反馈B设计盘问", project: "agent-nexus", file: "__sessions/260615-1958-原型反馈B设计盘问.md", size: "2.5 KB", updated: "2026-06-15 20:06", source: "local", excerpt: "围绕原型反馈B逐项澄清领域模型与页面交互边界。", body: "# 主题一\n\n## 设计决策\n- Project Key 默认取项目目录名，MVP 中 UI 不提供编辑入口。\n- Backup / Distribution / Restore/Pull 的方向与类型定义在 Task 层，不在 Task Group 层。\n- Task 的定时器（CRON）配置绑定在 Task 层，并支持创建后单独编辑。\n- Provider 卡片、Task Group、Task 都支持拖拽排序。\n- Session 主界面只显示 Cloud；WebDAV 仅出现在设置页面。" },
  { id: "s3", title: "260615-1430-prototype-build-notes", project: "agent-nexus", file: "__sessions/260615-1430-prototype-build-notes.md", size: "3.1 KB", updated: "2026-06-15 14:30", source: "local", excerpt: "Working notes while wiring up the six primary screens and the agent matrix interaction.", body: "# Prototype build notes\n\n- Top tab bar nav, native desktop chrome.\n- Agent Matrix collapsed to per-agent icons (source / target / none).\n- Session: Local/Cloud toggle, list + preview two-column.\n- Sync default sections moved to the bottom, collapsed." },
  { id: "s4", title: "260612-1432-tap-builder-refactor", project: "oll-context", file: "__sessions/260612-1432-tap-builder-refactor.md", size: "8.7 KB", updated: "2026-06-12 14:32", source: "both", excerpt: "Refactored the TAP builder skill and re-symlinked the canonical source into CodeX.", body: "# tap-builder refactor\n\n- Moved canonical source under .github/skills/tap-builder.\n- Re-created the CodeX placement as a symlink (auto target path).\n- disable-model-invocation left off for this skill." },
  { id: "s5", title: "260610-0901-quota-endpoint-debug", project: "agent-nexus", file: "__sessions/260610-0901-quota-endpoint-debug.md", size: "5.4 KB", updated: "2026-06-10 09:01", source: "cloud", excerpt: "Traced the DeepSeek 503 returned from the quota endpoint and added a retry path.", body: "# Quota endpoint debug\n\n- DeepSeek quota endpoint intermittently returns HTTP 503.\n- Provider card now surfaces \"Request failed\" with the last-success timestamp.\n- Added Retry as a diagnostic action." },
  { id: "s6", title: "260608-1730-session-archive-design", project: "tap-kit", file: "__sessions/260608-1730-session-archive-design.md", size: "6.9 KB", updated: "2026-06-08 17:30", source: "cloud", excerpt: "Designed the Cloud archive layout keyed by stable project key, no device sub-layer.", body: "# Session archive design\n\n- Cloud layout keyed by <project key>.\n- No <device-id> sub-layer; same project dir is a merge point, not a conflict.\n- last-write-wins on same-name files." },
  { id: "s7", title: "260605-1100-warp-config-sync", project: "awesome-vibe-coding", file: "docs/__sessions/260605-1100-warp-config-sync.md", size: "4.2 KB", updated: "2026-06-05 11:00", source: "cloud", excerpt: "Set up Warp config distribution from the canonical machine via copy, same-platform only.", body: "# Warp config sync\n\n- Generic File task from the Warp Config template.\n- Copies settings.toml + keybindings.yaml to Cloud.\n- Same-platform restore only." },
  { id: "s8", title: "260611-0815-local-scratch", project: "tap", file: "__sessions/260611-0815-local-scratch.md", size: "1.2 KB", updated: "2026-06-11 08:15", source: "local", excerpt: "Quick local scratch notes, not yet archived to Cloud.", body: "# Local scratch\n\n- Local-only notes for the tap repo.\n- Not yet archived — visible under Local source only." },
];

// ─── Sync · Task Groups ─────────────────────────────────────────────────────
// Direction / action / schedule live on each Task, not on the Group.

const TASK_GROUPS: TaskGroup[] = [
  { id: "g1", name: "Warp Config", tasks: [
    { id: "t1", direction: "Push", action: "Copy", sourceType: "Local", source: "~/.config/warp/settings.toml", targetType: "Cloud", target: "config/warp/settings.toml", schedule: "0 5 * * *", lastRun: "06-15 05:00", status: "ok", linkState: "present" },
    { id: "t2", direction: "Push", action: "Copy", sourceType: "Local", source: "~/.config/warp/keybindings.yaml", targetType: "Cloud", target: "config/warp/keybindings.yaml", schedule: "0 5 * * *", lastRun: "06-15 05:00", status: "ok", linkState: "present" },
  ] },
  { id: "g2", name: "TAP symlinks", tasks: [
    { id: "t3", direction: "Distribution", action: "Symlink", sourceType: "Local", source: "D:/Workspace/tap/src", targetType: "Local", target: "oll-context/backend/", schedule: "manual", lastRun: "06-14 18:02", status: "ok", linkState: "present" },
    { id: "t4", direction: "Distribution", action: "Symlink", sourceType: "Local", source: "D:/Workspace/tap/src", targetType: "Local", target: "oll-context/frontend/", schedule: "manual", lastRun: "06-14 18:02", status: "ok", linkState: "present" },
  ] },
  { id: "g3", name: "Machine Backup", tasks: [
    { id: "t5", direction: "Push", action: "Copy", sourceType: "Local", source: "~/.ssh/", targetType: "Cloud", target: "backups/ssh/", schedule: "0 3 * * 0", lastRun: "—", status: "never", linkState: "present" },
    { id: "t6", direction: "Pull", action: "Copy", sourceType: "Cloud", source: "config/zed/", targetType: "Local", target: "%APPDATA%/Zed/", schedule: "manual", lastRun: "06-13 09:10", status: "ok", linkState: "present" },
  ] },
];

// ─── Sync · Templates instantiate whole Task Groups ─────────────────────────

const TEMPLATES: Template[] = [
  { id: "blank", name: "Blank", desc: "Start an empty group and add tasks yourself.", tasks: [] },
  { id: "machine", name: "Machine Backup", desc: "Push SSH keys + pull editor config back from Cloud.", tasks: [
    { action: "Copy", sourceType: "Local", source: "~/.ssh/", targetType: "Cloud", target: "backups/ssh/", schedule: "0 3 * * 0" },
    { action: "Copy", sourceType: "Cloud", source: "config/zed/", targetType: "Local", target: "%APPDATA%/Zed/", schedule: "manual" },
  ] },
  { id: "warp", name: "Warp Config", desc: "Push Warp settings + keybindings to Cloud (two tasks).", tasks: [
    { action: "Copy", sourceType: "Local", source: "~/.config/warp/settings.toml", targetType: "Cloud", target: "config/warp/settings.toml", schedule: "0 5 * * *" },
    { action: "Copy", sourceType: "Local", source: "~/.config/warp/keybindings.yaml", targetType: "Cloud", target: "config/warp/keybindings.yaml", schedule: "0 5 * * *" },
  ] },
  { id: "zed", name: "Zed Config", desc: "Push Zed keymap + settings to Cloud (macOS, two tasks).", tasks: [
    { action: "Copy", sourceType: "Local", source: "~/.config/zed/keymap.json", targetType: "Cloud", target: "Zed/keymap.macos.json", schedule: "manual" },
    { action: "Copy", sourceType: "Local", source: "~/.config/zed/settings.json", targetType: "Cloud", target: "Zed/settings.macos.json", schedule: "manual" },
  ] },
  { id: "dotfiles", name: "Dotfiles", desc: "Distribute shared dotfiles locally, then push them to Cloud.", tasks: [
    { action: "Symlink", sourceType: "Local", source: "~/dotfiles/", targetType: "Local", target: "~/", schedule: "manual" },
    { action: "Copy", sourceType: "Local", source: "~/dotfiles/", targetType: "Cloud", target: "backups/dotfiles/", schedule: "0 4 * * *" },
  ] },
];

// ─── Sync · system-managed default records (read-only) ──────────────────────

const SYSTEM_SYNC: SystemSync = {
  skill: [
    { asset: "tap-builder", relation: "Claude Code → CodeX", path: "~/.codex/skills/tap-builder", status: "ok" },
    { asset: "commit-helper", relation: "Claude Code → OpenCode", path: "~/.config/opencode/skills/commit-helper", status: "pending" },
    { asset: "commit-helper", relation: "Claude Code → Copilot", path: "~/.github/skills/commit-helper", status: "ok" },
  ],
  prompt: [
    { asset: "Global Instructions", relation: "Claude Code → CodeX", path: "~/.codex/AGENTS.md", status: "ok" },
    { asset: "Global Instructions", relation: "Claude Code → Copilot", path: "~/.github/AGENTS.md", status: "ok" },
  ],
  backup: [
    { asset: "oll-context", relation: "__sessions → Cloud", path: "cloud://agent-nexus/oll-context", status: "ok" },
    { asset: "agent-nexus", relation: "__sessions → Cloud", path: "cloud://agent-nexus/agent-nexus", status: "pending" },
    { asset: "tap", relation: "__sessions → Cloud", path: "cloud://agent-nexus/tap", status: "never" },
  ],
};

// ─── Settings ───────────────────────────────────────────────────────────────

const SETTINGS: Settings = {
  webdav: {
    url: "https://nas.local/webdav/agent-nexus",
    user: "nexus",
    pass: "",
    remoteRoot: "agent-nexus-sync",
    status: "ok",
  },
  trayMetric: "Remaining",
};

// ─── Public API (mirrors prototype window.NexusData) ────────────────────────

export const nexus = {
  scanBase: SCAN_BASE,
  gitBaseFolders: (): GitBaseFolder[] => clone(GIT_BASE_FOLDERS),
  providers: (): Provider[] => clone(PROVIDERS),
  projects: (): Project[] => clone(PROJECTS),
  project: (id: string): Project | null => {
    const p = PROJECTS.find((x) => x.id === id);
    return p ? clone(p) : null;
  },
  skills: (): Skill[] => clone(SKILLS),
  prompts: (): Prompt[] => clone(PROMPTS),
  sessions: (): Session[] => clone(SESSIONS),
  sessionsForProject: (id: string, source: SessionSource): Session[] =>
    clone(SESSIONS.filter((se) => se.project === id && (se.source === source || se.source === "both"))),
  scanResults: (): ScanResult[] => clone(SCAN_RESULTS),
  taskGroups: (): TaskGroup[] => clone(TASK_GROUPS),
  templates: (): Template[] => clone(TEMPLATES),
  systemSync: (): SystemSync => clone(SYSTEM_SYNC),
  settings: (): Settings => clone(SETTINGS),
};
