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
    const { invoke } = await import("@tauri-apps/api/core");
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
