import type { InputHTMLAttributes, ReactNode } from "react";
import { Input } from "@/components/ui/primitives";

/** Vertical stack for one provider's connection fields. */
export function ConnectionFields({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-[13px]">{children}</div>;
}

/** Label + Input + hint row shared by every connection form. */
export function ConnectionField({
  label,
  hint,
  ...inputProps
}: { label: string; hint: ReactNode } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="block">
      <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">{label}</div>
      <Input {...inputProps} />
      <div className="mt-[5px] text-[11px] text-[#b3a999]">{hint}</div>
    </label>
  );
}
