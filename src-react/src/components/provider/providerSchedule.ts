/**
 * Per-provider scheduling settings surfaced inside the ⚙️ Configure modal:
 *  - quota refresh cadence (drives the front-end react-query poll interval)
 *  - window alignment (a daily local start time plus 5-hour cadence that fires a
 *    minimal billable request so the rolling quota window resets predictably)
 *
 * Window alignment is active only when BOTH a start time and a model are set; leaving
 * either blank turns it off.
 */

export const DEFAULT_QUOTA_REFRESH_MINUTES = 5;

export const QUOTA_REFRESH_PRESETS: { label: string; minutes: number }[] = [
  { label: "1 min", minutes: 1 },
  { label: "5 min", minutes: 5 },
  { label: "15 min", minutes: 15 },
  { label: "30 min", minutes: 30 },
  { label: "1 hour", minutes: 60 },
];

export const WINDOW_ALIGN_START_TIME_PRESETS: { label: string; value: string }[] = [
  { label: "04:00", value: "04:00" },
  { label: "05:00", value: "05:00" },
  { label: "06:00", value: "06:00" },
  { label: "07:00", value: "07:00" },
  { label: "08:00", value: "08:00" },
];

/** Quota polling is interval-natured; convert the configured minutes to react-query ms. */
export function quotaRefreshIntervalMs(minutes: number | null | undefined): number {
  const value = typeof minutes === "number" && Number.isFinite(minutes) ? minutes : DEFAULT_QUOTA_REFRESH_MINUTES;
  return Math.max(1, Math.floor(value)) * 60_000;
}

const pad2 = (n: number) => n.toString().padStart(2, "0");

export function windowAlignStartTimeToCron(value: string): string {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value.trim());
  if (!match) return "";
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (!Number.isInteger(hour) || !Number.isInteger(minute) || hour > 23 || minute > 59) {
    return "";
  }
  return `${minute} ${hour} * * *`;
}

export function windowAlignCronToStartTime(expr: string): string {
  const match = /^(\d{1,2})\s+([\d,]+)\s+\*\s+\*\s+\*$/.exec(expr.trim());
  if (!match) return "";
  const minute = Number(match[1]);
  const hour = Number(match[2].split(",")[0]);
  if (!Number.isInteger(hour) || !Number.isInteger(minute) || hour > 23 || minute > 59) {
    return "";
  }
  return `${pad2(hour)}:${pad2(minute)}`;
}

export function windowAlignStartTimeHuman(value: string): string {
  const normalized = windowAlignCronToStartTime(windowAlignStartTimeToCron(value));
  if (!normalized) return "Add a local first trigger time and model to enable window alignment.";
  return `Every day starts at ${normalized} local time; later attempts follow the 5-hour window.`;
}

/** Window alignment fires only when both a start time and a model are present. */
export function isWindowAlignActive(startTime: string, modelId: string | null | undefined): boolean {
  return (
    windowAlignStartTimeToCron(startTime).length > 0 &&
    typeof modelId === "string" &&
    modelId.trim().length > 0
  );
}

export function windowAlignLastAttemptLabel(epochSeconds: number | null | undefined): string {
  if (typeof epochSeconds !== "number" || !Number.isFinite(epochSeconds) || epochSeconds <= 0) {
    return "Never triggered";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(epochSeconds * 1000));
}

export function windowAlignNextAttemptLabel(epochSeconds: number | null | undefined): string {
  if (typeof epochSeconds !== "number" || !Number.isFinite(epochSeconds) || epochSeconds <= 0) {
    return "Not scheduled";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(epochSeconds * 1000));
}

export function windowAlignStatusLabel(status: string | null | undefined): string {
  switch (status) {
    case "success":
      return "Success";
    case "retryable_failed":
      return "Temporary failure";
    case "terminal_failed":
      return "Failed";
    default:
      return "No result yet";
  }
}
