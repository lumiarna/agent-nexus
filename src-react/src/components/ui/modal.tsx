import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Panel width / max-height overrides (default w-[460px]). */
  className?: string;
  /** Overlay tweaks (e.g. darker scrim, higher z-index). */
  overlayClassName?: string;
}

/** Lightweight modal: portal + centered panel, closes on ESC or overlay click,
 *  locks body scroll while open. Markup mirrors the prototype's modals. */
export function Modal({
  open,
  onClose,
  children,
  className,
  overlayClassName,
}: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      onClick={onClose}
      className={cn(
        "fixed inset-0 z-[60] flex animate-ann-fade items-center justify-center bg-[rgba(42,33,28,.34)] p-6",
        overlayClassName,
      )}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className={cn(
          "w-[460px] max-w-full overflow-auto rounded-[20px] border border-nexus-border bg-nexus-card shadow-[0_24px_60px_rgba(50,40,25,.28)]",
          className,
        )}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}

/** Standard modal header: title + optional subtitle and close affordance. */
export function ModalHeader({
  title,
  subtitle,
  onClose,
  titleClassName,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  onClose?: () => void;
  titleClassName?: string;
}) {
  return (
    <div className="flex items-start justify-between gap-3 border-b border-nexus-panel px-[22px] py-5">
      <div>
        <div
          className={cn(
            "text-[17px] font-extrabold tracking-[-.01em] text-nexus-ink",
            titleClassName,
          )}
        >
          {title}
        </div>
        {subtitle ? (
          <div className="mt-1 text-[12px] text-[#a99a89]">{subtitle}</div>
        ) : null}
      </div>
      {onClose ? (
        <div
          onClick={onClose}
          className="cursor-pointer text-[20px] leading-none text-[#b3a999] hover:text-[#7a6f60]"
        >
          ×
        </div>
      ) : null}
    </div>
  );
}

export function ModalFooter({ children }: { children: ReactNode }) {
  return (
    <div className="flex justify-end gap-[9px] border-t border-nexus-panel px-[22px] py-4">
      {children}
    </div>
  );
}
