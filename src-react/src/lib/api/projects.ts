import type { GitBaseFolder, Project, ScanResult } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export const projectsApi = {
  list(): Promise<Project[]> {
    return invokeCommand<Project[]>("list_projects");
  },

  record(path: string): Promise<Project> {
    return invokeCommand<Project>("record_project", { path });
  },

  listGitBaseFolders(): Promise<GitBaseFolder[]> {
    return invokeCommand<GitBaseFolder[]>("list_git_base_folders");
  },

  recordGitBaseFolder(path: string): Promise<GitBaseFolder> {
    return invokeCommand<GitBaseFolder>("record_git_base_folder", { path });
  },

  removeGitBaseFolder(id: string): Promise<void> {
    return invokeCommand<void>("remove_git_base_folder", { id });
  },

  scanBaseFolder(path: string): Promise<ScanResult[]> {
    return invokeCommand<ScanResult[]>("scan_git_base_folder", { path });
  },

  scanGitBaseFolders(): Promise<ScanResult[]> {
    return invokeCommand<ScanResult[]>("scan_git_base_folders");
  },
};
