export interface ProviderQuotaWindowInput {
  label: string;
  used: number;
  valueLabel?: string | null;
  valueOnly?: boolean;
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
  valueOnly?: boolean;
  /** Time-elapsed share of the window as a 0–100 percent, for placing the pace
   *  marker. Only present for weekly/monthly windows with a usable resetAt. */
  pace?: number;
}

export interface ProviderQuotaDisplay {
  primaryLabel: string;
  primaryCaption: string;
  windows: ProviderQuotaDisplayWindow[];
}

const PACE_ALERT_THRESHOLD = 15;

export function isQuotaPaceAlert(
  window: Pick<ProviderQuotaDisplayWindow, "used" | "pace">,
): boolean {
  return window.pace != null && window.used - window.pace > PACE_ALERT_THRESHOLD;
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
    primaryCaption: provider.primary != null ? "shortest window used" : "",
    windows: (provider.windows ?? []).map((window) => {
      const valueOnly = window.valueOnly ?? false;
      const pace = computePace(window, options.now ?? new Date());
      return {
        label: window.label,
        usedLabel: formatWindowValue(window, options),
        used: window.used,
        reset: valueOnly ? "" : formatWindowReset(window, options),
        unlimited: window.unlimited ?? false,
        ...(valueOnly ? { valueOnly } : {}),
        ...(pace != null ? { pace } : {}),
      };
    }),
  };
}

const WEEKLY_WINDOW_MS = 7 * 24 * 60 * 60 * 1000;

function formatWindowValue(
  window: ProviderQuotaWindowInput,
  options: ProviderQuotaDisplayOptions,
): string {
  if (
    window.valueOnly &&
    window.resetAt &&
    (!window.valueLabel || window.valueLabel === "Available")
  ) {
    return formatLocalExpiryTime(window.resetAt, options.timeZone) ?? window.valueLabel ?? "Available";
  }
  return window.valueLabel ?? (window.unlimited ? "Unlimited" : `${window.used}%`);
}

function formatLocalExpiryTime(expiresAt: string, timeZone?: string): string | undefined {
  const expiry = new Date(expiresAt);
  if (Number.isNaN(expiry.getTime())) return undefined;

  const parts = new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  }).formatToParts(expiry);
  const month = parts.find((part) => part.type === "month")?.value;
  const day = parts.find((part) => part.type === "day")?.value;
  const hour = parts.find((part) => part.type === "hour")?.value;
  const minute = parts.find((part) => part.type === "minute")?.value;

  if (!month || !day || !hour || !minute) return undefined;
  return `Expires ${month} ${day} ${hour}:${minute}`;
}

/** Share of the window already elapsed, as a 0–100 percent, for placing the pace
 *  marker. Only weekly/monthly windows qualify: their length is a protocol fact
 *  (7 days / one calendar month) derivable exactly from resetAt. Short rolling
 *  windows are excluded — uniform burn doesn't hold over a few hours, so a marker
 *  there is noise rather than signal. */
function computePace(window: ProviderQuotaWindowInput, now: Date): number | undefined {
  if (window.valueOnly || window.unlimited) return undefined;
  if (window.kind !== "weekly" && window.kind !== "monthly") return undefined;
  if (!window.resetAt) return undefined;

  const reset = new Date(window.resetAt);
  if (Number.isNaN(reset.getTime())) return undefined;

  const start =
    window.kind === "weekly"
      ? new Date(reset.getTime() - WEEKLY_WINDOW_MS)
      : startOfPreviousMonth(reset);

  const total = reset.getTime() - start.getTime();
  if (total <= 0) return undefined;

  const fraction = (now.getTime() - start.getTime()) / total;
  return Math.min(1, Math.max(0, fraction)) * 100;
}

/** The calendar-month window's start: the same instant one month before reset. */
function startOfPreviousMonth(reset: Date): Date {
  const start = new Date(reset.getTime());
  start.setUTCMonth(start.getUTCMonth() - 1);
  return start;
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
