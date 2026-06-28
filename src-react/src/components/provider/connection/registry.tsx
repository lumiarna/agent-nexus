import type { ReactNode } from "react";
import { providersApi, type OpenCodeGoConnectionParams } from "@/lib/api/providers";
import { ApiKeyForm } from "./ApiKeyForm";
import { CopilotTokenForm } from "./CopilotTokenForm";
import { OpenCodeGoConnectionForm } from "./OpenCodeGoConnectionForm";

export interface ApiKeyProviderHint {
  placeholder: string;
  authKey: string;
  savedLabel: string;
}

/** API-key providers share one form, parameterized by these per-provider hints. */
const API_KEY_PROVIDER_HINTS: Record<string, ApiKeyProviderHint> = {
  "minimax-token": {
    placeholder: "sk-...",
    authKey: "minimax-cn-coding-plan",
    savedLabel: "MiniMax Token Plan CN API key",
  },
  deepseek: {
    placeholder: "sk-...",
    authKey: "deepseek",
    savedLabel: "DeepSeek API key",
  },
  openrouter: {
    placeholder: "sk-or-v1-...",
    authKey: "openrouter",
    savedLabel: "OpenRouter API key",
  },
};

/**
 * A provider's connection-parameter editor, lifted out of the form component so
 * the Configure modal can own the value and commit every section through its one
 * footer Save (no more per-form Save button). `load`/`save` bind to the matching
 * Tauri command; `render` is a controlled, button-less field group.
 *
 * Values are erased to `unknown` at this registry boundary and re-narrowed inside
 * each entry — the per-provider value shapes (string key vs. workspace+cookie)
 * don't share a type, and the modal only stores/forwards the opaque value.
 *
 * Adding a provider with connection params = one new form file + one entry here.
 */
export interface ConnectionEditor {
  /** Value shown before `load` resolves and when no runtime/credential exists. */
  empty: unknown;
  /** Read the persisted value (only called inside the desktop runtime). */
  load: () => Promise<unknown>;
  /** Persist the edited value. */
  save: (value: unknown) => Promise<void>;
  /** Render the controlled fields for `value`; `onChange` reports edits. */
  render: (value: unknown, onChange: (next: unknown) => void) => ReactNode;
}

const FIXED_EDITORS: Record<string, ConnectionEditor> = {
  copilot: {
    empty: "",
    load: () => providersApi.getCopilotGithubToken().then((token) => token ?? ""),
    save: (value) => providersApi.setCopilotGithubToken((value as string).trim()),
    render: (value, onChange) => (
      <CopilotTokenForm value={value as string} onChange={onChange} />
    ),
  },
  "opencode-go": {
    empty: { workspaceId: "", authCookie: "" } satisfies OpenCodeGoConnectionParams,
    load: () => providersApi.getOpenCodeGoConnectionParams(),
    save: (value) => {
      const params = value as OpenCodeGoConnectionParams;
      return providersApi.setOpenCodeGoConnectionParams({
        workspaceId: params.workspaceId.trim(),
        authCookie: params.authCookie.trim(),
      });
    },
    render: (value, onChange) => (
      <OpenCodeGoConnectionForm value={value as OpenCodeGoConnectionParams} onChange={onChange} />
    ),
  },
};

/**
 * Return the connection editor for `providerId`, or `null` when the provider
 * needs none (pure OAuth/keychain providers such as claude/codex).
 */
export function connectionEditorFor(providerId: string): ConnectionEditor | null {
  const fixed = FIXED_EDITORS[providerId];
  if (fixed) return fixed;

  const hint = API_KEY_PROVIDER_HINTS[providerId];
  if (hint) {
    return {
      empty: "",
      load: () =>
        providersApi.getProviderConnectionParams(providerId).then((params) => params.apiKey),
      save: (value) =>
        providersApi.setProviderConnectionParams(providerId, { apiKey: (value as string).trim() }),
      render: (value, onChange) => (
        <ApiKeyForm hint={hint} value={value as string} onChange={onChange} />
      ),
    };
  }

  return null;
}
