import { useEffect, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/primitives";
import { isTauriRuntime } from "@/lib/runtime";

/**
 * Local state for one provider's Provider Connection Params editor: owns the
 * input value + saving flag, loads the existing value once on mount, and runs
 * the injected `onSave`. `load` / `onSave` are injected (no Tauri import here),
 * so a form can be unit-tested with fakes and without a desktop runtime.
 */
export function useConnectionFormState<T>(
  initial: T,
  load: () => Promise<T>,
  onSave: (value: T) => Promise<void>,
) {
  const [value, setValue] = useState<T>(initial);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let active = true;
    load()
      .then((loaded) => {
        if (active) setValue(loaded);
      })
      .catch(() => {
        if (active) setValue(initial);
      });
    return () => {
      active = false;
    };
    // Load once per mount. The registry rebinds `load` per provider and the
    // modal keys the form by providerId, so switching providers remounts and
    // reloads — there is no dependency to track here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function save() {
    if (saving) return;
    setSaving(true);
    try {
      await onSave(value);
    } finally {
      setSaving(false);
    }
  }

  return { value, setValue, saving, save };
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

/** Field stack + the form-owned Save button. */
export function ConnectionSection({
  saving,
  onSave,
  children,
}: {
  saving: boolean;
  onSave: () => void;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-[13px]">
      {children}
      <div className="flex justify-end">
        <Button variant="subtle" size="sm" disabled={saving} onClick={onSave}>
          {saving ? "Saving..." : "Save connection"}
        </Button>
      </div>
    </div>
  );
}
