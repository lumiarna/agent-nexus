import { invokeCommand } from "@/lib/api/tauri";
import type { TrayMetric } from "@/types";

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

export interface QoderConnectionParams {
  sessionCookie: string;
}

export interface ProviderConnectionParams {
  apiKey: string;
}

export interface ProviderDisplayPreferences {
  cardVisibility: string[];
  trayMetric: TrayMetric;
  /** Provider ids shown as a Windows-taskbar tray icon (a Surface Preference,
   *  independent of card visibility). */
  trayVisibility: string[];
}

/** One desired tray icon: brand-coloured square + the metric number (0–100),
 *  or null for quota fetch failure. */
export interface TrayEntry {
  providerId: string;
  label: string;
  colorHex: string;
  value: number | null;
}

export interface ProviderScheduleSettings {
  /** Front-end quota poll cadence, in minutes. */
  quotaRefreshMinutes: number;
  /** Back-end window-alignment daily local cron; "" means off. */
  windowAlignCron: string;
  /** Model the alignment request uses; null/"" means off. */
  windowAlignModelId: string | null;
  /** Read-only: next scheduled attempt time (epoch seconds); null when off. */
  windowAlignNextAttemptAt?: number | null;
  windowAlignLastAttemptAt?: number | null;
  windowAlignLastStatus?: "never" | "success" | "retryable_failed" | "terminal_failed";
  windowAlignLastError?: string | null;
}

export interface ProviderTriggerModel {
  id: string;
  displayName: string;
}

export interface ProviderTriggerCapability {
  /** Whether the back-end implements window alignment for this provider. */
  supported: boolean;
  /** Dynamically listed models; empty when unsupported. */
  models: ProviderTriggerModel[];
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
  getDisplayPreferences(): Promise<ProviderDisplayPreferences> {
    return invokeCommand<ProviderDisplayPreferences>("get_provider_display_preferences");
  },
  setDisplayPreferences(
    preferences: ProviderDisplayPreferences,
  ): Promise<ProviderDisplayPreferences> {
    return invokeCommand<ProviderDisplayPreferences>("set_provider_display_preferences", {
      preferences,
    });
  },
  getQuota(providerId: string): Promise<ProviderQuotaSnapshot> {
    return invokeCommand<ProviderQuotaSnapshot>("get_provider_quota", { providerId });
  },
  syncTray(entries: TrayEntry[]): Promise<void> {
    return invokeCommand<void>("sync_tray", { entries });
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
  getQoderConnectionParams(): Promise<QoderConnectionParams> {
    return invokeCommand<QoderConnectionParams>("get_qoder_connection_params");
  },
  setQoderConnectionParams(params: QoderConnectionParams): Promise<void> {
    return invokeCommand<void>("set_qoder_connection_params", { params });
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
  getProviderScheduleSettings(providerId: string): Promise<ProviderScheduleSettings> {
    return invokeCommand<ProviderScheduleSettings>("get_provider_schedule_settings", {
      providerId,
    });
  },
  setProviderScheduleSettings(
    providerId: string,
    settings: ProviderScheduleSettings,
  ): Promise<ProviderScheduleSettings> {
    return invokeCommand<ProviderScheduleSettings>("set_provider_schedule_settings", {
      providerId,
      settings,
    });
  },
  listProviderTriggerModels(providerId: string): Promise<ProviderTriggerCapability> {
    return invokeCommand<ProviderTriggerCapability>("list_provider_trigger_models", {
      providerId,
    });
  },
  runProviderWindowAlignment(
    providerId: string,
    modelId: string,
  ): Promise<ProviderScheduleSettings> {
    return invokeCommand<ProviderScheduleSettings>("run_provider_window_alignment", {
      providerId,
      modelId,
    });
  },
};
