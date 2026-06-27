import { useState } from "react";
import { toast } from "sonner";
import {
  useGitBaseFoldersQuery,
  useRecordGitBaseFolderMutation,
  useRemoveGitBaseFolderMutation,
} from "@/lib/query/projects";
import { getErrorMessage } from "./getErrorMessage";

/**
 * Git Base Folder cluster: the discovery directories list plus add/remove. The
 * modal open state lives here; a changed folder set invalidates any prior scan
 * via the scan hook's own folder-count effect.
 */
export function useGitBaseFolders() {
  const foldersQuery = useGitBaseFoldersQuery();
  const recordFolder = useRecordGitBaseFolderMutation();
  const removeFolder = useRemoveGitBaseFolderMutation();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");

  const folders = foldersQuery.data ?? [];

  async function add() {
    const trimmed = path.trim();
    if (!trimmed) {
      toast("Git base folder path is required");
      return;
    }

    try {
      await recordFolder.mutateAsync(trimmed);
      setPath("");
      toast(`Added base folder · ${trimmed}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function remove(id: string, folderPath: string) {
    try {
      await removeFolder.mutateAsync(id);
      toast(`Removed base folder · ${folderPath}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return {
    open,
    setOpen,
    path,
    setPath,
    folders,
    add,
    remove,
    isLoading: foldersQuery.isLoading,
    isFetching: foldersQuery.isFetching,
    refetch: foldersQuery.refetch,
    error: foldersQuery.error ? getErrorMessage(foldersQuery.error) : null,
    isAdding: recordFolder.isPending,
    isRemoving: removeFolder.isPending,
  };
}
