import { useEffect, useState, type ReactNode } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Unexpected error";
}

export interface SingleValueConfigModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  label: string;
  /** Seed value for the input each time the modal opens. */
  initialValue: string;
  placeholder?: string;
  /**
   * Persist the trimmed value (empty means "restore default"); returns the
   * canonical stored value to echo back in the success toast.
   */
  onSubmit: (value: string) => Promise<string>;
  busy?: boolean;
  messages: {
    /** Built from the canonical value when a non-empty value was saved. */
    set: (canonical: string) => string;
    /** Shown when the value was cleared. */
    cleared: string;
  };
  help: ReactNode;
}

/**
 * Single-value sibling of {@link StringListConfigModal}: a prefilled input with
 * Save / Cancel where an empty value restores the default. Used by the Session
 * Directory editor, which is one value rather than a list.
 */
export function SingleValueConfigModal({
  open,
  onClose,
  title,
  subtitle,
  label,
  initialValue,
  placeholder,
  onSubmit,
  busy,
  messages,
  help,
}: SingleValueConfigModalProps) {
  const [input, setInput] = useState(initialValue);

  useEffect(() => {
    if (open) setInput(initialValue);
  }, [open, initialValue]);

  async function submit() {
    const value = input.trim();
    try {
      const canonical = await onSubmit(value);
      onClose();
      toast(value ? messages.set(canonical) : messages.cleared);
    } catch (error) {
      toast(errorMessage(error));
    }
  }

  return (
    <Modal open={open} onClose={onClose} className="w-[480px]">
      <ModalHeader title={title} subtitle={subtitle} />
      <div className="flex flex-col gap-4 px-[22px] py-5">
        <div>
          <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">{label}</div>
          <input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !busy) void submit();
            }}
            placeholder={placeholder}
            className="w-full rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055] outline-none focus:border-nexus-accent"
            autoFocus
          />
        </div>
        <div className="rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-[11px] text-[11.5px] leading-[1.55] text-[#8a7a68]">
          {help}
        </div>
      </div>
      <ModalFooter>
        <Button variant="subtle" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" onClick={() => void submit()} disabled={busy}>
          {busy ? "Saving..." : "Save"}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
