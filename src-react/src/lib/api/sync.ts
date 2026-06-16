import type { ProjectSymlink } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export const syncApi = {
  listProjectSymlinks(): Promise<ProjectSymlink[]> {
    return invokeCommand<ProjectSymlink[]>("list_project_symlinks");
  },
};
