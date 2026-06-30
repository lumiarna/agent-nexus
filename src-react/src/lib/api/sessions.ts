import type { Session } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export const sessionsApi = {
  listLocal(): Promise<Session[]> {
    return invokeCommand<Session[]>("list_local_sessions");
  },

  listCloud(): Promise<Session[]> {
    return invokeCommand<Session[]>("list_cloud_sessions");
  },

  getLocal(id: string): Promise<Session> {
    return invokeCommand<Session>("get_local_session", { id });
  },

  openLocalSource(id: string): Promise<void> {
    return invokeCommand<void>("open_local_session_source", { id });
  },

  getCloud(id: string): Promise<Session> {
    return invokeCommand<Session>("get_cloud_session", { id });
  },

  scanLocal(): Promise<Session[]> {
    return invokeCommand<Session[]>("scan_local_sessions");
  },

  scanCloud(): Promise<Session[]> {
    return invokeCommand<Session[]>("scan_cloud_sessions");
  },
};
