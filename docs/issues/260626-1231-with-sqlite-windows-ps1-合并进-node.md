# with-sqlite Windows 工具链合并为单份 Node 脚本

## 问题

Windows 下的 SQLite 动态链接工具链分散在两份文件、两种语言：

- `scripts/with-sqlite.mjs`：Node 入口，负责跨平台分发（非 Windows 直接透传命令）。
- `scripts/with-sqlite-windows.ps1`：PowerShell，干所有实活——下载 `sqlite-dll-win-x64-*.zip`、校验 SHA256、解压、用 MSVC `lib.exe` 从 `sqlite3.def` 生成 `sqlite3.lib`、设环境变量、拷 `sqlite3.dll` 到 `target/{debug,release}`、再跑实际命令。

两者之间靠 base64 编码传参（`with-sqlite.mjs:13` 编码，`with-sqlite-windows.ps1:175-181` 解码），这层编解码**纯粹是为跨越 JS→PowerShell 进程边界而存在的偶然复杂度**。而 `.ps1` 里没有任何 PowerShell 独有的能力，全部可由 Node 完成。

## 决策

**暂不做。** 理由：

- `.ps1` 约 200 行、稳定、隔离良好，无当前痛点，churn 风险低。
- 收益（单语言 + 删掉 base64 桥）属中等，不是非做不可。
- 重写 `lib.exe` / `vswhere` 发现逻辑有在特定机器配置上引入回归的小概率，battle-tested 的 PowerShell 现成可用。

## 未来实现约定

端口化映射：

| `.ps1` 做的事 | Node 替代 | 难度 |
|---|---|---|
| `Invoke-WebRequest` 下载 | `fetch()`（Node 18+，自动跟随重定向） | 简单 |
| SHA256 校验 | `crypto.createHash('sha256')` | 简单 |
| `Expand-Archive` 解压 zip | 见下方决策点 | ⚠️ |
| `vswhere` + 递归找 `lib.exe` | `spawnSync(vswhere)` + `fs.readdirSync` 手动递归 | 偏繁琐 |
| `lib.exe /DEF` 生成 `.lib` | `spawnSync(libExe, ...)` | 简单 |
| 设 `$env:` 后跑命令 | `spawnSync(cmd, { env: { ...process.env, ... } })` | 简单 |
| 拷 `sqlite3.dll` 到 target | `fs.copyFileSync` | 简单 |

**唯一需要拍板的决策点 = 解压方案**（也是唯一会动 `package.json`/lockfile 的地方）：

- **方案 A（推荐，零新依赖）**：调 Windows 自带 `tar.exe`（bsdtar，Win10 1803+/11 必有）：`tar -xf archive.zip -C dest`。脚本本来就 shell out 调 `lib.exe`，多调一个系统自带 tar 不引入新的假设类别（已假定 MSVC 工具链在场，bsdtar 比 MSVC 普遍得多）。
- **方案 B（一个 npm devDependency）**：`yauzl` 或 `adm-zip`，纯进程内、显式自包含，符合 CLAUDE.md 第 4 条。压缩包有 SHA256 校验，`adm-zip` 历史上的路径穿越问题在此不成立。

> ⚠️ 纠错：不要用 `node:zlib` 解 `.zip`。`node:zlib` 只是 deflate/gzip 的**编解码器**，不认识 ZIP 容器格式（local file header / central directory）；且产物是 `.zip` 不是 `.tar.gz`，tar stdlib 也不对口。

影响面（删 `.ps1` 的 blast radius）：

- `package.json` 的 `rust:test` / `sqlite:setup` / `tauri` 三个 script **只调 `.mjs`**，不受影响。
- `.ps1` 仅被 `with-sqlite.mjs:28` 引用，无其它脚本/CI 直接调用。
- `docs/adr/0001-动态链接 SQLite.md:22` 点名了这个 `.ps1`，端口化后**需同步改这一行**。

落地步骤：改写 `with-sqlite.mjs` → 删 `with-sqlite-windows.ps1` → 更新 ADR 0001 line 22 → `pnpm sqlite:setup` 验证。

## 备注

- **收益是「单语言 + 删 base64 桥」，不是减行数。** `lib.exe`/`vswhere` 发现逻辑（vswhere 解析 + 递归 glob）正是 PowerShell 顺手、Node 繁琐的地方，搬到 Node 得手写 `fs` 递归，总行数大概持平甚至略增。若期待「代码少一半」，预期需调整。
- 尊重 ADR 0001「动态链接 SQLite」既定决策：端口化**不**借机改成 bundled/静态链接，仅更新文档对 `.ps1` 的描述。
