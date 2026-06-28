import { ConnectionField, ConnectionFields } from "./shared";

export interface CopilotTokenFormProps {
  value: string;
  onChange: (token: string) => void;
}

/**
 * Controlled Provider Connection Params editor for Copilot — a single GitHub
 * token. Stateless and button-less; the Configure modal commits it on Save.
 */
export function CopilotTokenForm({ value, onChange }: CopilotTokenFormProps) {
  return (
    <ConnectionFields>
      <ConnectionField
        label="GitHub token"
        type="password"
        className="font-mono"
        placeholder="gho_... or ghp_..."
        value={value}
        onChange={(event) => onChange(event.target.value)}
        hint={
          <>
            A Copilot-scoped GitHub token used only to read quota. Leave empty to fall back to
            opencode&apos;s <span className="font-mono">auth.json</span> (
            <span className="font-mono">github-copilot</span>).
          </>
        }
      />
    </ConnectionFields>
  );
}
