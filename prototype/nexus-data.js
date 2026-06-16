/* ============================================================
   Agent Nexus · data read/write boundary
   Single source of truth for seed data + shared lookups.
   Pages read from window.NexusData and clone what they mutate,
   so static data never lives inside page state logic.
   ============================================================ */
(function () {
  "use strict";

  var clone = function (x) { return JSON.parse(JSON.stringify(x)); };

  /* ---- Agents -------------------------------------------------
     Distribution / display order. "Agents" is the generic default
     placement (the shared ~/.agents config dir) and sits leftmost
     in the Agent Matrix. Real agents follow the canonical order
     Claude Code / CodeX / Copilot / OpenCode. ---------------- */
  var AGENT_ORDER = ["Agents", "Claude Code", "CodeX", "Copilot", "OpenCode"];
  var AGENT_META = {
    "Agents":      { abbr: "AG", color: "#9a7b53", configDir: "~/.agents",          generic: true },
    "Claude Code": { abbr: "CC", color: "#c2410c", configDir: "~/.claude" },
    "CodeX":       { abbr: "CX", color: "#4f7a6a", configDir: "~/.codex" },
    "Copilot":     { abbr: "CP", color: "#5a7894", configDir: "~/.github" },
    "OpenCode":    { abbr: "OC", color: "#7a5c9e", configDir: "~/.config/opencode" }
  };

  /* ---- Palette / status helpers (pure data) ---------------- */
  var C = { good: "#8a9a5b", warn: "#c2913f", crit: "#b55440", accent: "#9d7a64", muted: "#a99a89" };
  function quotaColor(u) { return u >= 90 ? C.crit : u >= 70 ? C.warn : C.good; }

  /* ---- Providers ------------------------------------------- */
  var PROVIDERS = [
    { id: "claude", name: "Claude Code", plan: "Max 20×", status: "available", credential: "~/.claude", primary: 68, isAgent: true,
      windows: [ { label: "5-hour limit", used: 42, reset: "Resets in 2h 14m" }, { label: "Weekly limit", used: 68, reset: "Resets Mon 09:00" } ] },
    { id: "codex", name: "CodeX", plan: "ChatGPT Plus", status: "available", credential: "~/.codex/auth.json", primary: 51, isAgent: true,
      windows: [ { label: "5-hour limit", used: 23, reset: "Resets in 3h 40m" }, { label: "Weekly limit", used: 51, reset: "Resets Thu 14:00" } ] },
    { id: "copilot", name: "Copilot", plan: "Business", status: "available", credential: "$GITHUB_TOKEN", primary: 77, isAgent: true,
      windows: [ { label: "Premium requests (monthly)", used: 77, reset: "Resets Jul 1" } ] },
    { id: "opencode-go", name: "OpenCode Go", plan: "Workspace", status: "expired", credential: "manual · workspace id + cookie", needsParams: true },
    { id: "minimax", name: "MiniMax CN", plan: "Pay-as-you-go", status: "available", credential: "~/.local/share/opencode/auth.json", primary: 12,
      windows: [ { label: "Daily tokens", used: 12, reset: "Resets 00:00 CST" } ] },
    { id: "deepseek", name: "DeepSeek", plan: "—", status: "failed", credential: "~/.local/share/opencode/auth.json", error: "HTTP 503 from quota endpoint. Last success 41m ago." },
    { id: "openrouter", name: "OpenRouter", plan: "Credits", status: "available", credential: "~/.local/share/opencode/auth.json", primary: 34,
      windows: [ { label: "Credit balance", used: 34, reset: "$66.00 of $100.00 left" } ] },
    { id: "minimax-token", name: "MiniMax Token Plan CN", plan: "Token plan", status: "nocreds", credential: "not found", hiddenCard: true }
  ];

  /* ---- Projects (Project Key == folder name, no edit) ------ */
  var PROJECTS = [
    { id: "oll-context", name: "oll-context", status: "active", path: "D:/Workspace/oll-context", sessionsDir: "__sessions", skills: 2, sessions: 18, sync: 6 },
    { id: "tap", name: "tap", status: "active", path: "D:/Workspace/tap", sessionsDir: "__sessions", skills: 1, sessions: 7, sync: 3 },
    { id: "tap-kit", name: "tap-kit", status: "active", path: "D:/Workspace/tap-kit", sessionsDir: "__sessions", skills: 0, sessions: 3, sync: 2 },
    { id: "awesome-vibe-coding", name: "awesome-vibe-coding", status: "active", path: "D:/Workspace/awesome-vibe-coding", sessionsDir: "docs/__sessions", sessionsNote: " · override", skills: 1, sessions: 5, sync: 2 },
    { id: "agent-nexus", name: "agent-nexus", status: "active", path: "D:/Workspace/agent-nexus", sessionsDir: "__sessions", skills: 3, sessions: 12, sync: 4 },
    { id: "legacy-cli", name: "legacy-cli", status: "stale", path: "E:/Old/legacy-cli", sessionsDir: "__sessions", skills: 1, sessions: 9, sync: 2 },
    { id: "sandbox", name: "sandbox", status: "hidden", path: "D:/Workspace/sandbox", sessionsDir: "__sessions", skills: 0, sessions: 2, sync: 0 }
  ];
  // Project Key always derives from the folder name (UI offers no editor).
  PROJECTS.forEach(function (p) { p.key = p.name; });

  /* ---- Discoverable repos for "Add Git Base Folder" scan --- */
  var SCAN_BASE = "D:/Workspace";
  var SCAN_RESULTS = [
    { path: "D:/Workspace/oll-context", key: "oll-context", state: "recorded" },
    { path: "D:/Workspace/tap", key: "tap", state: "recorded" },
    { path: "D:/Workspace/new-service", key: "new-service", state: "new" },
    { path: "D:/Workspace/billing-api", key: "billing-api", state: "new" },
    { path: "D:/Workspace/docs-site", key: "docs-site", state: "new" }
  ];

  /* ---- Skills (cells keyed by agent name; one source per row) */
  var SKILLS = [
    { id: "sk1", name: "tap-builder", scope: "global", desc: "Scaffold TAP modules from a spec file", path: "~/.claude/skills/tap-builder", disabled: false,
      cells: { "Agents": "none", "Claude Code": "source", "CodeX": "target", "Copilot": "target", "OpenCode": "none" } },
    { id: "sk2", name: "commit-helper", scope: "global", desc: "Generate conventional commit messages", path: "~/.claude/skills/commit-helper", disabled: false,
      cells: { "Agents": "target", "Claude Code": "source", "CodeX": "target", "Copilot": "target", "OpenCode": "target" } },
    { id: "sk3", name: "issue-triage", scope: "global", desc: "Apply canonical triage labels to issues", path: "~/.config/opencode/skills/issue-triage", disabled: true,
      cells: { "Agents": "none", "Claude Code": "none", "CodeX": "none", "Copilot": "none", "OpenCode": "source" } },
    { id: "sk4", name: "pdf-extractor", scope: "global", desc: "Extract structured data from PDFs", path: "~/.codex/skills/pdf-extractor", disabled: false,
      cells: { "Agents": "target", "Claude Code": "none", "CodeX": "source", "Copilot": "none", "OpenCode": "target" } },
    { id: "sk5", name: "domain-doctor", scope: "global", desc: "Maintain CONTEXT.md domain docs", path: "~/.claude/skills/domain-doctor", disabled: false,
      cells: { "Agents": "none", "Claude Code": "source", "CodeX": "none", "Copilot": "target", "OpenCode": "none" } },
    { id: "sk6", name: "tap-builder", scope: "project", projectId: "oll-context", desc: "Project-scoped TAP scaffolder", path: ".github/skills/tap-builder", disabled: false,
      cells: { "Agents": "none", "Claude Code": "source", "CodeX": "target", "Copilot": "target", "OpenCode": "none" } },
    { id: "sk7", name: "test-runner", scope: "project", projectId: "oll-context", desc: "Run and summarize the test suite", path: ".codex/skills/test-runner", disabled: false,
      cells: { "Agents": "none", "Claude Code": "none", "CodeX": "source", "Copilot": "none", "OpenCode": "none" } },
    { id: "sk8", name: "release-notes", scope: "project", projectId: "tap", desc: "Draft release notes from merged PRs", path: ".claude/skills/release-notes", disabled: false,
      cells: { "Agents": "none", "Claude Code": "source", "CodeX": "none", "Copilot": "target", "OpenCode": "none" } },
    { id: "sk9", name: "vibe-lint", scope: "project", projectId: "awesome-vibe-coding", desc: "Lint vibe-coding examples", path: ".agents/skills/vibe-lint", disabled: false,
      cells: { "Agents": "target", "Claude Code": "none", "CodeX": "none", "Copilot": "none", "OpenCode": "source" } },
    { id: "sk10", name: "context-doctor", scope: "project", projectId: "agent-nexus", desc: "Maintain CONTEXT.md domain docs", path: ".claude/skills/context-doctor", disabled: false,
      cells: { "Agents": "none", "Claude Code": "source", "CodeX": "target", "Copilot": "none", "OpenCode": "none" } }
  ];

  /* ---- Prompts (global single-file assets) ----------------- */
  var PROMPTS = [
    { id: "pr1", name: "Global Instructions", path: "~/.claude/CLAUDE.md",
      cells: { "Agents": "target", "Claude Code": "source", "CodeX": "target", "Copilot": "target", "OpenCode": "target" } },
    { id: "pr2", name: "Agent Conventions", path: "~/.codex/AGENTS.md",
      cells: { "Agents": "target", "Claude Code": "none", "CodeX": "source", "Copilot": "none", "OpenCode": "target" } }
  ];

  /* ---- Sessions -------------------------------------------- */
  var SESSIONS = [
    { id: "s1", title: "260615-1059-Agent Nexus需求审视", project: "agent-nexus", file: "__sessions/260615-1059-Agent Nexus需求审视.md", size: "11.3 KB", updated: "2026-06-15 13:19", source: "both",
      excerpt: "审视基于当前项目演化出的新项目需求边界、实体模型与范围优先级。",
      body: "# 主题一\n\n## 设计决策\n- 先不讨论实现，优先澄清产品定位、范围边界、实体关系与同步语义。\n- 一级导航命名最终确认：单数形式 Provider / Project / Skill / Prompt / Session / Sync。\n- Sync 语义确认：只做单向，仅支持 Backup 与 Distribution 两种单向模式。\n- Agent Matrix 作为高层快捷控制器，用户在表格内直接切换关系。" },
    { id: "s2", title: "260615-1958-原型反馈B设计盘问", project: "agent-nexus", file: "__sessions/260615-1958-原型反馈B设计盘问.md", size: "2.5 KB", updated: "2026-06-15 20:06", source: "local",
      excerpt: "围绕原型反馈B逐项澄清领域模型与页面交互边界。",
      body: "# 主题一\n\n## 设计决策\n- Project Key 默认取项目目录名，MVP 中 UI 不提供编辑入口。\n- Backup / Distribution / Restore/Pull 的方向与类型定义在 Task 层，不在 Task Group 层。\n- Task 的定时器（CRON）配置绑定在 Task 层，并支持创建后单独编辑。\n- Provider 卡片、Task Group、Task 都支持拖拽排序。\n- Session 主界面只显示 Cloud；WebDAV 仅出现在设置页面。" },
    { id: "s3", title: "260615-1430-prototype-build-notes", project: "agent-nexus", file: "__sessions/260615-1430-prototype-build-notes.md", size: "3.1 KB", updated: "2026-06-15 14:30", source: "local",
      excerpt: "Working notes while wiring up the six primary screens and the agent matrix interaction.",
      body: "# Prototype build notes\n\n- Top tab bar nav, native desktop chrome.\n- Agent Matrix collapsed to per-agent icons (source / target / none).\n- Session: Local/Cloud toggle, list + preview two-column.\n- Sync default sections moved to the bottom, collapsed." },
    { id: "s4", title: "260612-1432-tap-builder-refactor", project: "oll-context", file: "__sessions/260612-1432-tap-builder-refactor.md", size: "8.7 KB", updated: "2026-06-12 14:32", source: "both",
      excerpt: "Refactored the TAP builder skill and re-symlinked the canonical source into CodeX.",
      body: "# tap-builder refactor\n\n- Moved canonical source under .github/skills/tap-builder.\n- Re-created the CodeX placement as a symlink (auto target path).\n- disable-model-invocation left off for this skill." },
    { id: "s5", title: "260610-0901-quota-endpoint-debug", project: "agent-nexus", file: "__sessions/260610-0901-quota-endpoint-debug.md", size: "5.4 KB", updated: "2026-06-10 09:01", source: "cloud",
      excerpt: "Traced the DeepSeek 503 returned from the quota endpoint and added a retry path.",
      body: "# Quota endpoint debug\n\n- DeepSeek quota endpoint intermittently returns HTTP 503.\n- Provider card now surfaces \"Request failed\" with the last-success timestamp.\n- Added Retry as a diagnostic action." },
    { id: "s6", title: "260608-1730-session-archive-design", project: "tap-kit", file: "__sessions/260608-1730-session-archive-design.md", size: "6.9 KB", updated: "2026-06-08 17:30", source: "cloud",
      excerpt: "Designed the Cloud archive layout keyed by stable project key, no device sub-layer.",
      body: "# Session archive design\n\n- Cloud layout keyed by <project key>.\n- No <device-id> sub-layer; same project dir is a merge point, not a conflict.\n- last-write-wins on same-name files." },
    { id: "s7", title: "260605-1100-warp-config-sync", project: "awesome-vibe-coding", file: "docs/__sessions/260605-1100-warp-config-sync.md", size: "4.2 KB", updated: "2026-06-05 11:00", source: "cloud",
      excerpt: "Set up Warp config distribution from the canonical machine via copy, same-platform only.",
      body: "# Warp config sync\n\n- Generic File task from the Warp Config template.\n- Copies settings.toml + keybindings.yaml to Cloud.\n- Same-platform restore only." },
    { id: "s8", title: "260611-0815-local-scratch", project: "tap", file: "__sessions/260611-0815-local-scratch.md", size: "1.2 KB", updated: "2026-06-11 08:15", source: "local",
      excerpt: "Quick local scratch notes, not yet archived to Cloud.",
      body: "# Local scratch\n\n- Local-only notes for the tap repo.\n- Not yet archived — visible under Local source only." }
  ];

  /* ---- Sync · Task Groups -----------------------------------
     Direction (Distribution / Backup / Restore-Pull), action and
     schedule live on each Task, not on the Group. ------------- */
  var TASK_GROUPS = [
    { id: "g1", name: "Warp Config", tasks: [
      { id: "t1", direction: "Backup", action: "copy", source: "~/.config/warp/settings.toml", targets: ["webdav://nas/config/warp/"], schedule: "0 5 * * *", lastRun: "06-15 05:00", status: "ok" },
      { id: "t2", direction: "Backup", action: "copy", source: "~/.config/warp/keybindings.yaml", targets: ["webdav://nas/config/warp/"], schedule: "0 5 * * *", lastRun: "06-15 05:00", status: "ok" }
    ] },
    { id: "g2", name: "TAP symlinks", tasks: [
      { id: "t3", direction: "Distribution", action: "symlink", source: "D:/Workspace/tap/src", targets: ["oll-context/backend/", "oll-context/frontend/"], schedule: "manual", lastRun: "06-14 18:02", status: "ok" }
    ] },
    { id: "g3", name: "Machine Backup", tasks: [
      { id: "t5", direction: "Backup", action: "copy", source: "~/.ssh/", targets: ["webdav://nas/backups/ssh/"], schedule: "0 3 * * 0", lastRun: "—", status: "never" },
      { id: "t6", direction: "Restore/Pull", action: "copy", source: "webdav://nas/config/zed/", targets: ["%APPDATA%/Zed/"], schedule: "manual", lastRun: "06-13 09:10", status: "ok" }
    ] }
  ];

  /* ---- Sync · Templates instantiate whole Task Groups ------ */
  var TEMPLATES = [
    { id: "blank", name: "Blank", desc: "Start an empty group and add tasks yourself.",
      tasks: [] },
    { id: "machine", name: "Machine Backup", desc: "Back up SSH keys + pull editor config back from Cloud.",
      tasks: [
        { direction: "Backup", action: "copy", source: "~/.ssh/", targets: ["webdav://nas/backups/ssh/"], schedule: "0 3 * * 0" },
        { direction: "Restore/Pull", action: "copy", source: "webdav://nas/config/zed/", targets: ["%APPDATA%/Zed/"], schedule: "manual" }
      ] },
    { id: "warp", name: "Warp Config", desc: "Back up Warp settings + keybindings to Cloud (two tasks).",
      tasks: [
        { direction: "Backup", action: "copy", source: "~/.config/warp/settings.toml", targets: ["webdav://nas/config/warp/"], schedule: "0 5 * * *" },
        { direction: "Backup", action: "copy", source: "~/.config/warp/keybindings.yaml", targets: ["webdav://nas/config/warp/"], schedule: "0 5 * * *" }
      ] },
    { id: "dotfiles", name: "Dotfiles", desc: "Distribute shared dotfiles across machines, then back them up.",
      tasks: [
        { direction: "Distribution", action: "symlink", source: "~/dotfiles/", targets: ["~/", "~/work/"], schedule: "manual" },
        { direction: "Backup", action: "copy", source: "~/dotfiles/", targets: ["webdav://nas/backups/dotfiles/"], schedule: "0 4 * * *" }
      ] }
  ];

  /* ---- Sync · system-managed default records (read-only) --- */
  var SYSTEM_SYNC = {
    skill: [
      { asset: "tap-builder", relation: "Claude Code → CodeX", path: "~/.codex/skills/tap-builder", status: "ok" },
      { asset: "commit-helper", relation: "Claude Code → OpenCode", path: "~/.config/opencode/skills/commit-helper", status: "pending" },
      { asset: "commit-helper", relation: "Claude Code → Copilot", path: "~/.github/skills/commit-helper", status: "ok" }
    ],
    prompt: [
      { asset: "Global Instructions", relation: "Claude Code → CodeX", path: "~/.codex/AGENTS.md", status: "ok" },
      { asset: "Global Instructions", relation: "Claude Code → Copilot", path: "~/.github/AGENTS.md", status: "ok" }
    ],
    backup: [
      { asset: "oll-context", relation: "__sessions → Cloud", path: "cloud://agent-nexus/oll-context", status: "ok" },
      { asset: "agent-nexus", relation: "__sessions → Cloud", path: "cloud://agent-nexus/agent-nexus", status: "pending" },
      { asset: "tap", relation: "__sessions → Cloud", path: "cloud://agent-nexus/tap", status: "never" }
    ]
  };

  /* ---- Settings -------------------------------------------- */
  var SETTINGS = {
    webdav: { url: "https://nas.local/webdav/agent-nexus", user: "nexus", pass: "", status: "ok" },
    trayMetric: "Remaining"
  };
  // Agent config roots, derived from AGENT_META in canonical order.
  function agentConfigRoots() {
    return AGENT_ORDER.map(function (name) {
      var m = AGENT_META[name];
      var root = m.configDir;
      var keyBase = name === "Agents" ? "AGENTS" : name.toUpperCase().replace(/[^A-Z]/g, "");
      return {
        name: name, generic: !!m.generic,
        dirs: [
          { envKey: keyBase + "_CONFIG_DIR", value: root },
          { envKey: keyBase + "_SKILLS_DIR", value: root + "/skills", derived: true },
          { envKey: keyBase + "_PROMPT_FILE", value: root + "/" + (name === "Claude Code" ? "CLAUDE.md" : "AGENTS.md"), derived: true }
        ]
      };
    });
  }

  /* ---- Public API ------------------------------------------ */
  window.NexusData = {
    AGENT_ORDER: AGENT_ORDER.slice(),
    AGENT_META: clone(AGENT_META),
    agentAbbr: function (a) { return (AGENT_META[a] || {}).abbr || "?"; },
    agentColor: function (a) { return (AGENT_META[a] || {}).color || "#a99a89"; },
    palette: clone(C),
    quotaColor: quotaColor,
    scanBase: SCAN_BASE,

    providers: function () { return clone(PROVIDERS); },
    projects: function () { return clone(PROJECTS); },
    project: function (id) { var p = PROJECTS.find(function (x) { return x.id === id; }); return p ? clone(p) : null; },
    skills: function () { return clone(SKILLS); },
    skillsForProject: function (id) { return clone(SKILLS.filter(function (k) { return k.scope === "project" && k.projectId === id; })); },
    prompts: function () { return clone(PROMPTS); },
    sessions: function () { return clone(SESSIONS); },
    sessionsForProject: function (id, source) {
      return clone(SESSIONS.filter(function (se) { return se.project === id && (se.source === source || se.source === "both"); }));
    },
    scanResults: function () { return clone(SCAN_RESULTS); },
    taskGroups: function () { return clone(TASK_GROUPS); },
    templates: function () { return clone(TEMPLATES); },
    systemSync: function () { return clone(SYSTEM_SYNC); },
    settings: function () { return clone(SETTINGS); },
    agentConfigRoots: agentConfigRoots
  };

  try { window.dispatchEvent(new Event("nexus-data-ready")); } catch (e) {}
})();
