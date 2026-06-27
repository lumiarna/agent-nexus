import { useState } from "react";
import { toast } from "sonner";
import { useDeleteProjectMutation } from "@/lib/query/projects";
import type { Project } from "@/types";
import { getErrorMessage } from "./getErrorMessage";
import { canConfirmDelete } from "./projectInteractions";

/**
 * Deletion cluster: the two-step confirm flow (`deleteId` + `acknowledged`)
 * over the existing delete mutation. The destructive button is gated by
 * {@link canConfirmDelete}, kept in one place.
 */
export function useProjectDeletion(projects: Project[]) {
  const deleteProject = useDeleteProjectMutation();
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);

  const target = deleteId ? projects.find((p) => p.id === deleteId) ?? null : null;

  function request(id: string) {
    setDeleteId(id);
    setAcknowledged(false);
  }

  function cancel() {
    setDeleteId(null);
    setAcknowledged(false);
  }

  function confirm() {
    if (!target || !canConfirmDelete(acknowledged, deleteProject.isPending)) return;
    const { id, name } = target;
    cancel();
    deleteProject.mutateAsync(id).then(
      () => toast(`${name} permanently deleted`),
      (error: unknown) => toast(getErrorMessage(error)),
    );
  }

  return {
    target,
    acknowledged,
    toggleAck: () => setAcknowledged((a) => !a),
    request,
    cancel,
    confirm,
    canConfirm: canConfirmDelete(acknowledged, deleteProject.isPending),
    isPending: deleteProject.isPending,
  };
}
