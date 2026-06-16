import type { Project } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export const projectsApi = {
  list(): Promise<Project[]> {
    return invokeCommand<Project[]>("list_projects");
  },

  record(path: string): Promise<Project> {
    return invokeCommand<Project>("record_project", { path });
  },
};
