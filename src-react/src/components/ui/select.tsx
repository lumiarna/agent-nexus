import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}

/**
 * Lightweight styled dropdown matching the paper design system. The menu is
 * portalled to <body> so it is never clipped by a scrolling modal panel, and
 * positioned under the trigger; it closes on outside click, outside scroll, or ESC.
 */
export function Select({
  value,
  onChange,
  options,
  placeholder = "Select…",
  disabled,
  className,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [rect, setRect] = useState<{ left: number; top: number; width: number } | null>(null);
  const selected = options.find((option) => option.value === value);

  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const bounds = triggerRef.current.getBoundingClientRect();
    setRect({ left: bounds.left, top: bounds.bottom + 4, width: bounds.width });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    const closeOnOutsideScroll = (event: Event) => {
      const target = event.target;
      if (
        target instanceof Node &&
        (menuRef.current?.contains(target) || triggerRef.current?.contains(target))
      ) {
        return;
      }
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    // Reposition would drift on scroll/resize, so collapse instead.
    window.addEventListener("scroll", closeOnOutsideScroll, true);
    window.addEventListener("resize", close);
    document.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("scroll", closeOnOutsideScroll, true);
      window.removeEventListener("resize", close);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        onClick={() => setOpen((prev) => !prev)}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-[10px] border border-nexus-border2 bg-white px-3 py-[9px] text-left text-[12.5px] outline-none transition-colors",
          disabled
            ? "cursor-not-allowed opacity-60"
            : "cursor-pointer hover:border-nexus-accent",
          open && "border-nexus-accent",
          className,
        )}
      >
        <span className={cn("truncate", selected ? "text-nexus-body" : "text-[#b3a999]")}>
          {selected ? selected.label : placeholder}
        </span>
        <ChevronDown
          size={15}
          className={cn("flex-none text-[#b3a999] transition-transform", open && "rotate-180")}
        />
      </button>
      {open && !disabled && rect
        ? createPortal(
            <>
              <div className="fixed inset-0 z-[70]" onClick={() => setOpen(false)} />
              <div
                ref={menuRef}
                className="fixed z-[71] max-h-[240px] overflow-auto rounded-[12px] border border-nexus-border2 bg-nexus-card p-1 shadow-[0_12px_32px_rgba(50,40,25,.18)]"
                style={{ left: rect.left, top: rect.top, width: rect.width }}
              >
                {options.length === 0 ? (
                  <div className="px-3 py-2 text-[12px] text-[#b3a999]">No options</div>
                ) : (
                  options.map((option) => {
                    const on = option.value === value;
                    return (
                      <div
                        key={option.value}
                        onClick={() => {
                          onChange(option.value);
                          setOpen(false);
                        }}
                        className={cn(
                          "flex cursor-pointer items-center justify-between gap-2 rounded-[8px] px-2.5 py-[7px] text-[12.5px] transition-colors",
                          on
                            ? "bg-nexus-sand font-semibold text-nexus-accent"
                            : "text-nexus-body hover:bg-nexus-sand",
                        )}
                      >
                        <span className="truncate">{option.label}</span>
                        {on ? <Check size={14} className="flex-none text-nexus-accent" /> : null}
                      </div>
                    );
                  })
                )}
              </div>
            </>,
            document.body,
          )
        : null}
    </>
  );
}
