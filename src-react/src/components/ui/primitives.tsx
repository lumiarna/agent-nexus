import { forwardRef, type HTMLAttributes, type InputHTMLAttributes } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

/** The standard `#fcf9f4` rounded, bordered surface used across pages. */
export function Card({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "rounded-[18px] border border-nexus-border bg-nexus-card shadow-[0_1px_14px_rgba(50,40,25,.05)]",
        className,
      )}
      {...props}
    />
  );
}

/** Small status dot. Color is runtime-computed, so it comes via `style`. */
export function Dot({
  color,
  dim = 8,
  pulse,
  className,
}: {
  color: string;
  dim?: number;
  pulse?: boolean;
  className?: string;
}) {
  return (
    <span
      className={cn("inline-block flex-none rounded-full", pulse && "animate-ann-pulse", className)}
      style={{ width: dim, height: dim, background: color }}
    />
  );
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "w-full rounded-[10px] border border-nexus-border2 bg-white px-3 py-[9px] text-[12.5px] text-nexus-body outline-none placeholder:text-[#b3a999]",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";

/** Muted spinning loader for in-progress fetches. */
export function Spinner({ size = 14, className }: { size?: number; className?: string }) {
  return <Loader2 size={size} className={cn("animate-spin text-[#b3a999]", className)} />;
}
