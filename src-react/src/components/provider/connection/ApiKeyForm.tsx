import { ConnectionField, ConnectionSection, useConnectionFormState } from "./shared";
import type { ApiKeyProviderHint } from "./registry";

export interface ApiKeyFormProps {
  hint: ApiKeyProviderHint;
  load: () => Promise<string>;
  onSave: (apiKey: string) => Promise<void>;
}

/** Provider Connection Params editor for API-key providers (DeepSeek, OpenRouter, …). */
export function ApiKeyForm({ hint, load, onSave }: ApiKeyFormProps) {
  const { value, setValue, saving, save } = useConnectionFormState("", load, onSave);
  return (
    <ConnectionSection saving={saving} onSave={() => void save()}>
      <ConnectionField
        label="API key"
        type="password"
        className="font-mono"
        placeholder={hint.placeholder}
        value={value}
        onChange={(event) => setValue(event.target.value)}
        hint={
          <>
            Used only to read quota. Leave empty to fall back to opencode&apos;s{" "}
            <span className="font-mono">auth.json</span> (
            <span className="font-mono">{hint.authKey}</span>).
          </>
        }
      />
    </ConnectionSection>
  );
}
