import type { OpenCodeGoConnectionParams } from "@/lib/api/providers";
import { ConnectionField, ConnectionFields } from "./shared";

export interface OpenCodeGoConnectionFormProps {
  value: OpenCodeGoConnectionParams;
  onChange: (params: OpenCodeGoConnectionParams) => void;
}

/**
 * Controlled Provider Connection Params editor for OpenCode Go — workspace id +
 * auth cookie. Stateless and button-less; the Configure modal commits it on Save.
 */
export function OpenCodeGoConnectionForm({ value, onChange }: OpenCodeGoConnectionFormProps) {
  return (
    <ConnectionFields>
      <ConnectionField
        label="Workspace ID"
        className="font-mono"
        placeholder="wrk_xxxxxxxxxxxx"
        value={value.workspaceId}
        onChange={(event) => onChange({ ...value, workspaceId: event.target.value })}
        hint="Required to query the workspace quota endpoint."
      />
      <ConnectionField
        label="Auth Cookie"
        type="password"
        className="font-mono"
        placeholder="Fe26.2**..."
        value={value.authCookie}
        onChange={(event) => onChange({ ...value, authCookie: event.target.value })}
        hint="Paste only the auth cookie value from opencode.ai."
      />
    </ConnectionFields>
  );
}
