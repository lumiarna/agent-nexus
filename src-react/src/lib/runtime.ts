import { invoke } from "@tauri-apps/api/core";

type DesktopHealthPayload = {
  ok: boolean;
  appName: string;
  appVersion: string;
};

export type DesktopHealthState =
  | { status: "unknown" }
  | { status: "unavailable" }
  | {
      status: "connected";
      ok: true;
      appName: string;
      appVersion: string;
    };

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function detectDesktopHealth(): Promise<DesktopHealthState> {
  if (!isTauriRuntime()) {
    return { status: "unavailable" };
  }

  try {
    const health = await invoke<DesktopHealthPayload>("get_desktop_health");

    if (!health.ok) {
      return { status: "unavailable" };
    }

    return {
      status: "connected",
      ok: true,
      appName: health.appName,
      appVersion: health.appVersion,
    };
  } catch {
    return { status: "unavailable" };
  }
}

export type HostPlatform = "windows" | "macos" | "linux" | "unknown";

/** Host OS, used to gate platform-only affordances (e.g. the Windows-only Junction action).
 *  Uses the desktop `get_platform` command when available; falls back to UA sniffing in the
 *  browser preview, which is reliable enough for hiding an action. */
export async function detectPlatform(): Promise<HostPlatform> {
  if (isTauriRuntime()) {
    try {
      const os = await invoke<string>("get_platform");
      if (os === "windows" || os === "macos" || os === "linux") {
        return os;
      }
      return "unknown";
    } catch {
      return "unknown";
    }
  }

  const ua = navigator.userAgent;
  if (ua.includes("Win")) return "windows";
  if (ua.includes("Mac")) return "macos";
  if (ua.includes("Linux")) return "linux";
  return "unknown";
}
