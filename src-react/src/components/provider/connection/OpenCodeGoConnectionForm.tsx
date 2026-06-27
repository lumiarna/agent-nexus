import type { OpenCodeGoConnectionParams } from "@/lib/api/providers";
import { ConnectionField, ConnectionSection, useConnectionFormState } from "./shared";

export interface OpenCodeGoConnectionFormProps {
  load: () => Promise<OpenCodeGoConnectionParams>;
  onSave: (params: OpenCodeGoConnectionParams) => Promise<void>;
}

/** Provider Connection Params editor for OpenCode Go — workspace id + auth cookie. */
export function OpenCodeGoConnectionForm({ load, onSave }: OpenCodeGoConnectionFormProps) {
  const { value, setValue, saving, save } = useConnectionFormState<OpenCodeGoConnectionParams>(
    { workspaceId: "", authCookie: "" },
    load,
    onSave,
  );
  return (
    <ConnectionSection saving={saving} onSave={() => void save()}>
      <ConnectionField
        label="Workspace ID"
        className="font-mono"
        placeholder="wrk_xxxxxxxxxxxx"
        value={value.workspaceId}
        onChange={(event) =>
          setValue((current) => ({ ...current, workspaceId: event.target.value }))
        }
        hint="Required to query the workspace quota endpoint."
      />
      <ConnectionField
        label="Auth Cookie"
        type="password"
        className="font-mono"
        placeholder="Fe26.2**..."
        value={value.authCookie}
        onChange={(event) =>
          setValue((current) => ({ ...current, authCookie: event.target.value }))
        }
        hint="Paste only the auth cookie value from opencode.ai."
      />
    </ConnectionSection>
  );
}
