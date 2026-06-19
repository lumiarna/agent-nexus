import type { ProjectSymlink } from "../../types/index.js";
import { invokeCommand } from "./tauri.js";

export const projectSymlinksApi = {
  list(): Promise<ProjectSymlink[]> {
    return invokeCommand<ProjectSymlink[]>("list_project_symlinks");
  },

  delete(targetPath: string): Promise<void> {
    return invokeCommand<void>("delete_project_symlink", { targetPath });
  },
};
