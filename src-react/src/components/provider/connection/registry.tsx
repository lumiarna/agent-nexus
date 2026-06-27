import type { ReactNode } from "react";
import { toast } from "sonner";
import { providersApi } from "@/lib/api/providers";
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

export interface ConnectionContext {
  /** Re-poll the provider's quota after its connection params change. */
  refreshProvider: (providerId: string) => void;
}

/**
 * `providerId → connection form` registry (the front-end mirror of the back-end
 * `provider_quota_adapters()` registry). Each entry binds the form's injected
 * `load` / `onSave` to the matching Tauri command; the form components stay free
 * of provider branching and of Tauri imports.
 *
 * Adding a provider with connection params = one new form file + one entry here.
 */
const FIXED_FORMS: Record<string, (ctx: ConnectionContext) => ReactNode> = {
  copilot: (ctx) => (
    <CopilotTokenForm
      key="copilot"
      load={() => providersApi.getCopilotGithubToken().then((token) => token ?? "")}
      onSave={async (token) => {
        try {
          await providersApi.setCopilotGithubToken(token.trim());
        } catch {
          toast.error("Failed to save Copilot GitHub token");
          return;
        }
        toast("Saved Copilot GitHub token");
        ctx.refreshProvider("copilot");
      }}
    />
  ),
  "opencode-go": (ctx) => (
    <OpenCodeGoConnectionForm
      key="opencode-go"
      load={() => providersApi.getOpenCodeGoConnectionParams()}
      onSave={async ({ workspaceId, authCookie }) => {
        try {
          await providersApi.setOpenCodeGoConnectionParams({
            workspaceId: workspaceId.trim(),
            authCookie: authCookie.trim(),
          });
        } catch {
          toast.error("Failed to save OpenCode Go connection params");
          return;
        }
        toast("Saved OpenCode Go connection params");
        ctx.refreshProvider("opencode-go");
      }}
    />
  ),
};

/**
 * Render the Provider Connection Params form for `providerId`, or `null` when
 * the provider needs none (pure OAuth/keychain providers such as claude/codex).
 */
export function connectionFormFor(
  providerId: string,
  ctx: ConnectionContext,
): ReactNode | null {
  const fixed = FIXED_FORMS[providerId];
  if (fixed) return fixed(ctx);

  const hint = API_KEY_PROVIDER_HINTS[providerId];
  if (hint) {
    return (
      <ApiKeyForm
        key={providerId}
        hint={hint}
        load={() =>
          providersApi.getProviderConnectionParams(providerId).then((params) => params.apiKey)
        }
        onSave={async (apiKey) => {
          try {
            await providersApi.setProviderConnectionParams(providerId, { apiKey: apiKey.trim() });
          } catch {
            toast.error(`Failed to save ${hint.savedLabel}`);
            return;
          }
          toast(`Saved ${hint.savedLabel}`);
          ctx.refreshProvider(providerId);
        }}
      />
    );
  }

  return null;
}
