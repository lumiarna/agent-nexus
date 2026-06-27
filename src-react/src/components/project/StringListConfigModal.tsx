import { useEffect, useState, type ReactNode } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { cn } from "@/lib/utils";
import { getErrorMessage } from "./getErrorMessage";
import { resolveAdd, resolveRemove } from "./stringListEdit";

export interface StringListConfigModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  /** Current entries; the source of truth lives in the parent. */
  items: string[];
  /** Persist the next list when an entry is added (parent injects mutateAsync). */
  onAdd: (next: string[]) => Promise<unknown>;
  /** Persist the next list when an entry is removed. */
  onRemove: (next: string[]) => Promise<unknown>;
  /** Return an error string to reject a value, or `null` to accept it. */
  validate?: (value: string) => string | null;
  placeholder?: string;
  /** Label for the add button, e.g. "Add dir" / "Add file". */
  addLabel: string;
  /** Seed value for the input each time the modal opens (default ""). */
  initialInput?: string;
  /** True while a mutation is in flight — disables the input and buttons. */
  busy?: boolean;
  messages: {
    added: (value: string) => string;
    removed: (value: string) => string;
    duplicate: string;
  };
  /** Shown when the list is empty. */
  emptyHint: ReactNode;
  /** Optional per-row badge (e.g. External / In repo, owner Agent). */
  renderBadge?: (value: string) => ReactNode;
  /** Explanatory footer paragraph. */
  help: ReactNode;
}

/**
 * Deep presentational editor for a Project custom string-list source. It hides
 * the add behavior — trim, `validate`, dedup, clear-input, toast — behind a
 * small props surface, so the skills-dirs and prompt-files modals are two
 * instances of one component instead of two hand-written copies.
 */
export function StringListConfigModal({
  open,
  onClose,
  title,
  subtitle,
  items,
  onAdd,
  onRemove,
  validate,
  placeholder,
  addLabel,
  initialInput = "",
  busy,
  messages,
  emptyHint,
  renderBadge,
  help,
}: StringListConfigModalProps) {
  const [input, setInput] = useState(initialInput);

  // Reseed the input each time the modal opens.
  useEffect(() => {
    if (open) setInput(initialInput);
  }, [open, initialInput]);

  async function add() {
    const outcome = resolveAdd(input, items, validate);
    if (outcome.kind === "empty") return;
    if (outcome.kind === "invalid") {
      toast(outcome.message);
      return;
    }
    if (outcome.kind === "duplicate") {
      toast(messages.duplicate);
      return;
    }

    try {
      await onAdd(outcome.next);
      setInput("");
      toast(messages.added(outcome.value));
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function remove(value: string) {
    try {
      await onRemove(resolveRemove(value, items));
      toast(messages.removed(value));
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return (
    <Modal open={open} onClose={onClose} className="max-h-[88vh] w-[560px]">
      <ModalHeader title={title} subtitle={subtitle} />
      <div className="flex flex-col gap-4 px-[22px] py-5">
        <div className="flex gap-2">
          <input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !busy) void add();
            }}
            placeholder={placeholder}
            className="min-w-0 flex-1 rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055] outline-none focus:border-nexus-accent"
          />
          <Button
            variant="secondary"
            size="sm"
            className="rounded-[10px]"
            onClick={() => void add()}
            disabled={busy || !input.trim()}
          >
            {busy ? "Saving..." : addLabel}
          </Button>
        </div>

        <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
          {items.length === 0 ? (
            <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">{emptyHint}</div>
          ) : (
            items.map((value, i) => (
              <div
                key={value}
                className={cn(
                  "flex items-center justify-between gap-3 px-3.5 py-[11px]",
                  i > 0 && "border-t border-[#f3eee5]",
                )}
              >
                <div className="flex min-w-0 items-center gap-2">
                  <span className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-nexus-body">
                    {value}
                  </span>
                  {renderBadge?.(value)}
                </div>
                <button
                  onClick={() => void remove(value)}
                  disabled={busy}
                  className="flex-none text-[11px] font-semibold text-nexus-crit hover:underline disabled:cursor-wait disabled:opacity-60"
                >
                  Remove
                </button>
              </div>
            ))
          )}
        </div>

        <div className="rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-[11px] text-[11.5px] leading-[1.55] text-[#8a7a68]">
          {help}
        </div>
      </div>
      <ModalFooter>
        <Button variant="subtle" onClick={onClose}>
          Close
        </Button>
      </ModalFooter>
    </Modal>
  );
}
