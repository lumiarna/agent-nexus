/**
 * Per-provider scheduling settings surfaced inside the ⚙️ Configure modal:
 *  - quota refresh cadence (drives the front-end react-query poll interval)
 *  - window alignment (a back-end cron that fires a minimal billable request so the
 *    rolling quota window resets on the user's schedule)
 *
 * Window alignment is active only when BOTH a cron and a model are set; leaving
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

export const WINDOW_ALIGN_CRON_PRESETS: { label: string; expr: string }[] = [
  { label: "05·10·15·20", expr: "0 5,10,15,20 * * *" },
  { label: "08·13·18·23", expr: "0 8,13,18,23 * * *" },
];

/** Quota polling is interval-natured; convert the configured minutes to react-query ms. */
export function quotaRefreshIntervalMs(minutes: number | null | undefined): number {
  const value = typeof minutes === "number" && Number.isFinite(minutes) ? minutes : DEFAULT_QUOTA_REFRESH_MINUTES;
  return Math.max(1, Math.floor(value)) * 60_000;
}

const pad2 = (n: number) => n.toString().padStart(2, "0");

/** Describe a `minute hours * * *` daily cron; otherwise stay honest about being custom/empty. */
export function windowAlignCronHuman(expr: string): string {
  const trimmed = expr.trim();
  if (!trimmed) return "Add a time and model to enable window alignment.";
  const match = /^(\d{1,2})\s+([\d,]+)\s+\*\s+\*\s+\*$/.exec(trimmed);
  if (!match) return "Custom schedule expression.";
  const minute = Number(match[1]);
  if (minute > 59) return "Custom schedule expression.";
  const hours = match[2].split(",").map(Number);
  if (hours.some((hour) => Number.isNaN(hour) || hour > 23)) return "Custom schedule expression.";
  const times = hours.map((hour) => `${pad2(hour)}:${pad2(minute)}`).join(" · ");
  return `Every day at ${times}.`;
}

/** Window alignment fires only when both a cron and a model are present. */
export function isWindowAlignActive(cron: string, modelId: string | null | undefined): boolean {
  return cron.trim().length > 0 && typeof modelId === "string" && modelId.trim().length > 0;
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
