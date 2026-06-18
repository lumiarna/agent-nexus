import { invoke } from "@tauri-apps/api/core";

import { isTauriRuntime } from "@/lib/runtime";

export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntime()) {
    throw new Error("Agent Nexus desktop runtime is required for this action.");
  }

  return invoke<T>(command, args);
}
