import type { QoderConnectionParams } from "@/lib/api/providers";
import { ConnectionField, ConnectionFields } from "./shared";

export interface QoderConnectionFormProps {
  value: QoderConnectionParams;
  onChange: (params: QoderConnectionParams) => void;
}

/**
 * Controlled Provider Connection Params editor for Qoder personal accounts:
 * the `qoder_session_cookie` value the user copies out of qoder.com DevTools.
 * Stateless and button-less; the Configure modal commits it on Save.
 */
export function QoderConnectionForm({ value, onChange }: QoderConnectionFormProps) {
  return (
    <ConnectionFields>
      <ConnectionField
        label="Session cookie"
        type="password"
        className="font-mono"
        placeholder="MTc4Mjg2NzQ3NHx..."
        value={value.sessionCookie}
        onChange={(event) => onChange({ ...value, sessionCookie: event.target.value })}
        hint={
          <>
            Paste only the <code>qoder_session_cookie</code> value from the DevTools
            Application &gt; Cookies panel for qoder.com. Do not include other cookies.
          </>
        }
      />
    </ConnectionFields>
  );
}
