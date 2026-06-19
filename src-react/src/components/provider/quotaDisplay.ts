export interface ProviderQuotaWindowInput {
  label: string;
  used: number;
  reset?: string;
  resetAt?: string | null;
  kind?: "rolling" | "weekly" | "monthly" | string;
  unlimited?: boolean;
}

export interface ProviderQuotaInput {
  primary?: number | null;
  windows?: ProviderQuotaWindowInput[] | null;
}

export interface ProviderQuotaDisplayWindow {
  label: string;
  usedLabel: string;
  used: number;
  reset: string;
  unlimited: boolean;
}

export interface ProviderQuotaDisplay {
  primaryLabel: string;
  primaryCaption: string;
  windows: ProviderQuotaDisplayWindow[];
}

interface ProviderQuotaDisplayOptions {
  now?: Date;
  timeZone?: string;
}

export function formatProviderQuotaDisplay(
  provider: ProviderQuotaInput,
  options: ProviderQuotaDisplayOptions = {},
): ProviderQuotaDisplay {
  return {
    primaryLabel: provider.primary != null ? `${provider.primary}%` : "",
    primaryCaption: "peak window used",
    windows: (provider.windows ?? []).map((window) => ({
      label: window.label,
      usedLabel: window.unlimited ? "Unlimited" : `${window.used}%`,
      used: window.used,
      reset: formatWindowReset(window, options),
      unlimited: window.unlimited ?? false,
    })),
  };
}

function formatWindowReset(
  window: ProviderQuotaWindowInput,
  options: ProviderQuotaDisplayOptions,
): string {
  if (!window.resetAt) return window.reset ?? "";

  if (window.kind === "rolling") {
    return formatRollingReset(window.resetAt, options.now ?? new Date());
  }

  if (window.kind === "monthly") {
    return formatMonthlyReset(window.resetAt);
  }

  return formatLocalResetTime(window.resetAt, options.timeZone);
}

function formatMonthlyReset(resetAt: string): string {
  const reset = new Date(resetAt);
  if (Number.isNaN(reset.getTime())) return "";

  const parts = new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  }).formatToParts(reset);
  const month = parts.find((part) => part.type === "month")?.value;
  const day = parts.find((part) => part.type === "day")?.value;

  if (!month || !day) return "";
  return `Resets ${month} ${day}`;
}

function formatRollingReset(resetAt: string, now: Date): string {
  const reset = new Date(resetAt);
  if (Number.isNaN(reset.getTime())) return "";

  const diffMs = reset.getTime() - now.getTime();
  if (diffMs <= 0) return "Resets now";

  const totalMinutes = Math.max(1, Math.floor(diffMs / 60_000));
  const days = Math.floor(totalMinutes / (24 * 60));
  const hours = Math.floor((totalMinutes % (24 * 60)) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return hours > 0 ? `Resets in ${days}d ${hours}h` : `Resets in ${days}d`;
  }
  if (hours > 0) {
    return minutes > 0 ? `Resets in ${hours}h ${minutes}m` : `Resets in ${hours}h`;
  }
  return `Resets in ${minutes}m`;
}

function formatLocalResetTime(resetAt: string, timeZone?: string): string {
  const reset = new Date(resetAt);
  if (Number.isNaN(reset.getTime())) return "";

  const parts = new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  }).formatToParts(reset);
  const weekday = parts.find((part) => part.type === "weekday")?.value;
  const hour = parts.find((part) => part.type === "hour")?.value;
  const minute = parts.find((part) => part.type === "minute")?.value;

  if (!weekday || !hour || !minute) return "";
  return `Resets ${weekday} ${hour}:${minute}`;
}
