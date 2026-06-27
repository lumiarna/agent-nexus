import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { cn } from "@/lib/utils";
import type { useProjectDeletion } from "./useProjectDeletion";

interface DeleteProjectModalProps {
  deletion: ReturnType<typeof useProjectDeletion>;
}

/** Two-step delete confirmation. The flow state and the destructive gate live
 *  in the injected `useProjectDeletion` hook. */
export function DeleteProjectModal({ deletion }: DeleteProjectModalProps) {
  const { target } = deletion;
  return (
    <Modal
      open={!!target}
      onClose={deletion.cancel}
      overlayClassName="bg-[rgba(42,33,28,.40)]"
    >
      {target ? (
        <>
          <ModalHeader
            title={`Delete ${target.name} permanently?`}
            titleClassName="text-nexus-crit"
            subtitle="This cannot be undone. The following associated data will be removed:"
          />
          <div className="px-[22px] py-[18px]">
            <div className="flex flex-col gap-0.5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-2">
              {[
                { label: "Archived & local sessions", value: `${target.sessions} files` },
                { label: "Project skills", value: `${target.skills}` },
                { label: "Sync tasks", value: `${target.sync}` },
                { label: "Session backup record", value: "1" },
              ].map((d) => (
                <div key={d.label} className="flex items-center justify-between gap-2.5 px-[11px] py-2">
                  <span className="text-[12.5px] text-[#6a5550]">{d.label}</span>
                  <span className="text-[12.5px] font-bold text-nexus-crit">{d.value}</span>
                </div>
              ))}
            </div>
            <label
              onClick={deletion.toggleAck}
              className="mt-3.5 flex cursor-pointer items-center gap-[9px]"
            >
              <span
                className="inline-flex h-[18px] w-[18px] flex-none items-center justify-center rounded-[5px] text-[11px] font-extrabold text-white"
                style={{
                  background: deletion.acknowledged ? "#b55440" : "#fff",
                  border: `1px solid ${deletion.acknowledged ? "#b55440" : "#d9c4b8"}`,
                }}
              >
                {deletion.acknowledged ? "✓" : ""}
              </span>
              <span className="text-[12.5px] text-[#6a6055]">
                I understand this permanently deletes the project and its data.
              </span>
            </label>
          </div>
          <ModalFooter>
            <Button variant="subtle" onClick={deletion.cancel}>
              Cancel
            </Button>
            <button
              onClick={deletion.confirm}
              className={cn(
                "rounded-full px-[18px] py-[9px] text-[12.5px] font-bold text-white",
                deletion.canConfirm
                  ? "cursor-pointer bg-nexus-crit shadow-[0_2px_8px_rgba(181,84,64,.32)]"
                  : "cursor-not-allowed bg-[#d9b6ab]",
              )}
            >
              {deletion.isPending ? "Deleting..." : "Delete permanently"}
            </button>
          </ModalFooter>
        </>
      ) : null}
    </Modal>
  );
}
