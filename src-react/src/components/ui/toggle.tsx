interface ToggleProps {
  checked: boolean;
  onChange: () => void;
  /** "warn" tints the active track amber (used for "disable invoke"). */
  tone?: "accent" | "warn";
  title?: string;
  disabled?: boolean;
}

/** Track + knob switch, sized to match the prototype (34×20, 16px knob). */
export function Toggle({ checked, onChange, tone = "accent", title, disabled }: ToggleProps) {
  const onColor = tone === "warn" ? "#c2913f" : "#9d7a64";
  return (
    <div
      onClick={disabled ? undefined : onChange}
      title={title}
      className={`relative h-5 w-[34px] flex-none rounded-full transition-colors ${disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer"}`}
      style={{ background: checked ? onColor : "#ddccb6" }}
    >
      <div
        className="absolute top-[2px] h-4 w-4 rounded-full bg-white transition-[left] shadow-[0_1px_2px_rgba(50,40,25,.3)]"
        style={{ left: checked ? "16px" : "2px" }}
      />
    </div>
  );
}
