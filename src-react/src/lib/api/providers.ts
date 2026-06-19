import { invokeCommand } from "@/lib/api/tauri";

export interface ProviderQuotaWindowSnapshot {
  label: string;
  kind: "rolling" | "weekly" | "monthly";
  used: number;
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

export const providersApi = {
  getQuota(providerId: string): Promise<ProviderQuotaSnapshot> {
    return invokeCommand<ProviderQuotaSnapshot>("get_provider_quota", { providerId });
  },
  getCopilotGithubToken(): Promise<string | null> {
    return invokeCommand<string | null>("get_copilot_github_token");
  },
  setCopilotGithubToken(token: string): Promise<void> {
    return invokeCommand<void>("set_copilot_github_token", { token });
  },
};
