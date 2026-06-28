import { ConnectionField, ConnectionFields } from "./shared";
import type { ApiKeyProviderHint } from "./registry";

export interface ApiKeyFormProps {
  hint: ApiKeyProviderHint;
  value: string;
  onChange: (apiKey: string) => void;
}

/**
 * Controlled Provider Connection Params editor for API-key providers (DeepSeek,
 * OpenRouter, …). Holds no state and has no Save button — the Configure modal
 * owns the value and commits it through its single footer Save.
 */
export function ApiKeyForm({ hint, value, onChange }: ApiKeyFormProps) {
  return (
    <ConnectionFields>
      <ConnectionField
        label="API key"
        type="password"
        className="font-mono"
        placeholder={hint.placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        hint={
          <>
            Used only to read quota. Leave empty to fall back to opencode&apos;s{" "}
            <span className="font-mono">auth.json</span> (
            <span className="font-mono">{hint.authKey}</span>).
          </>
        }
      />
    </ConnectionFields>
  );
}
