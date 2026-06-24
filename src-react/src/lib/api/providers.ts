import { invokeCommand } from "@/lib/api/tauri";

export interface ProviderQuotaWindowSnapshot {
  label: string;
  kind: "rolling" | "weekly" | "monthly";
  used: number;
  valueLabel?: string | null;
  valueOnly: boolean;
  resetAt?: string | null;
  unlimited: boolean;
}

export interface ProviderQuotaSnapshot {
  providerId: string;
  status: "available" | "expired" | "failed" | "nocreds";
  plan?: string | null;
  primary?: number | null;
  windows: ProviderQuotaWindowSnapshot[];
  credential?: string | null;
  error?: string | null;
}

export interface OpenCodeCustomProvider {
  id: string;
  name: string;
  npm: string;
  baseUrl: string;
  modelId: string;
}

export interface OpenCodeGoConnectionParams {
  workspaceId: string;
  authCookie: string;
}

export interface ProviderConnectionParams {
  apiKey: string;
}

export const providersApi = {
  listOpenCodeCustomProviders(): Promise<OpenCodeCustomProvider[]> {
    return invokeCommand<OpenCodeCustomProvider[]>("list_opencode_custom_providers");
  },
  getOrder(): Promise<string[]> {
    return invokeCommand<string[]>("get_provider_order");
  },
  setOrder(providerIds: string[]): Promise<string[]> {
    return invokeCommand<string[]>("set_provider_order", { providerIds });
  },
  getQuota(providerId: string): Promise<ProviderQuotaSnapshot> {
    return invokeCommand<ProviderQuotaSnapshot>("get_provider_quota", { providerId });
  },
  getCopilotGithubToken(): Promise<string | null> {
    return invokeCommand<string | null>("get_copilot_github_token");
  },
  setCopilotGithubToken(token: string): Promise<void> {
    return invokeCommand<void>("set_copilot_github_token", { token });
  },
  getOpenCodeGoConnectionParams(): Promise<OpenCodeGoConnectionParams> {
    return invokeCommand<OpenCodeGoConnectionParams>("get_opencode_go_connection_params");
  },
  setOpenCodeGoConnectionParams(params: OpenCodeGoConnectionParams): Promise<void> {
    return invokeCommand<void>("set_opencode_go_connection_params", { params });
  },
  getProviderConnectionParams(providerId: string): Promise<ProviderConnectionParams> {
    return invokeCommand<ProviderConnectionParams>("get_provider_connection_params", {
      providerId,
    });
  },
  setProviderConnectionParams(
    providerId: string,
    params: ProviderConnectionParams,
  ): Promise<void> {
    return invokeCommand<void>("set_provider_connection_params", { providerId, params });
  },
};
