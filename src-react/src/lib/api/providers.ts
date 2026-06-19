import { invokeCommand } from "@/lib/api/tauri";

export interface ProviderQuotaWindowSnapshot {
  label: string;
  kind: "rolling" | "weekly";
  used: number;
  resetAt?: string | null;
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
};
