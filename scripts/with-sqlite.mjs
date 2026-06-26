import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("usage: node scripts/with-sqlite.mjs [--setup-only | <command> [...args]]");
  process.exit(2);
}

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const commandArgsBase64 = args.map((arg) => Buffer.from(arg, "utf8").toString("base64")).join(",");

if (process.platform !== "win32" && args[0] === "--setup-only") {
  process.exit(0);
}

const result =
  process.platform === "win32"
    ? spawnSync(
        "pwsh.exe",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          resolve(repoRoot, "scripts", "with-sqlite-windows.ps1"),
          ...(args[0] === "--setup-only" ? ["-SetupOnly"] : ["-CommandArgsBase64", commandArgsBase64]),
        ],
        { cwd: repoRoot, stdio: "inherit" },
      )
    : spawnSync(args[0], args.slice(1), {
        cwd: repoRoot,
        stdio: "inherit",
      });

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
