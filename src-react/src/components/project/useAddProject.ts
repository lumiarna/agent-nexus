import { useState } from "react";
import { toast } from "sonner";
import { useRecordProjectMutation } from "@/lib/query/projects";
import type { Project } from "@/types";
import { getErrorMessage } from "./getErrorMessage";

/**
 * Add Project cluster: the modal open state, the path input, and the record
 * mutation. On success the caller decides where to navigate via `onRecorded`.
 */
export function useAddProject(onRecorded: (project: Project) => void) {
  const recordProject = useRecordProjectMutation();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");

  async function submit() {
    const trimmed = path.trim();
    if (!trimmed) {
      toast("Project path is required");
      return;
    }

    try {
      const project = await recordProject.mutateAsync(trimmed);
      setOpen(false);
      setPath("");
      toast(`Project recorded · key "${project.key}"`);
      onRecorded(project);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return {
    open,
    setOpen,
    path,
    setPath,
    submit,
    isPending: recordProject.isPending,
  };
}
