import { ConnectionField, ConnectionSection, useConnectionFormState } from "./shared";

export interface CopilotTokenFormProps {
  load: () => Promise<string>;
  onSave: (token: string) => Promise<void>;
}

/** Provider Connection Params editor for Copilot — a single GitHub token. */
export function CopilotTokenForm({ load, onSave }: CopilotTokenFormProps) {
  const { value, setValue, saving, save } = useConnectionFormState("", load, onSave);
  return (
    <ConnectionSection saving={saving} onSave={() => void save()}>
      <ConnectionField
        label="GitHub token"
        type="password"
        className="font-mono"
        placeholder="gho_... or ghp_..."
        value={value}
        onChange={(event) => setValue(event.target.value)}
        hint={
          <>
            A Copilot-scoped GitHub token used only to read quota. Leave empty to fall back to
            opencode&apos;s <span className="font-mono">auth.json</span> (
            <span className="font-mono">github-copilot</span>).
          </>
        }
      />
    </ConnectionSection>
  );
}
