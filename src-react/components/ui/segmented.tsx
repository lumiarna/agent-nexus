import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface SegOption<T extends string> {
  value: T;
  label: string;
}

interface SegmentedProps<T extends string> {
  options: SegOption<T>[];
  value: T;
  onChange: (v: T) => void;
  size?: "lg" | "md" | "sm";
  /** Container overrides — e.g. a different track background inside modals. */
  className?: string;
}

const SEG_SIZE = {
  lg: "px-[18px] py-[7px] text-[13px]",
  md: "px-[14px] py-[5px] text-[12px]",
  sm: "px-[11px] py-[5px] text-[11px]",
} as const;

/** Pill-group toggle with a "paper" active segment (Local/Cloud, scope, etc.). */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  size = "lg",
  className,
}: SegmentedProps<T>) {
  return (
    <div className={cn("flex rounded-full bg-nexus-panel p-[3px]", className)}>
      {options.map((o) => {
        const on = o.value === value;
        return (
          <div
            key={o.value}
            onClick={() => onChange(o.value)}
            className={cn(
              "cursor-pointer whitespace-nowrap rounded-full transition-colors",
              SEG_SIZE[size],
              on
                ? "bg-nexus-card font-bold text-nexus-accent shadow-[0_1px_2px_rgba(50,40,25,.06)]"
                : "font-semibold text-[#a99a89]",
            )}
          >
            {o.label}
          </div>
        );
      })}
    </div>
  );
}

interface ChipProps {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
  /** Monospace label (CRON presets). */
  mono?: boolean;
  className?: string;
  title?: string;
}

/** Filled-when-active pill (project filters, template/CRON-preset chips). */
export function Chip({ active, onClick, children, mono, className, title }: ChipProps) {
  return (
    <div
      onClick={onClick}
      title={title}
      className={cn(
        "cursor-pointer rounded-full border px-3 py-[5px] text-[12px] transition-colors",
        mono && "font-mono",
        active
          ? "border-nexus-accent bg-nexus-accent font-bold text-white"
          : "border-nexus-border2 bg-nexus-card font-semibold text-[#7a6f60] hover:bg-nexus-sand",
        className,
      )}
    >
      {children}
    </div>
  );
}
