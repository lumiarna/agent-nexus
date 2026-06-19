export const DEFAULT_CRON_SCHEDULE = "*/5 * * * *";

export const SCHEDULE_PRESETS = [
  { label: "Every 5 min", expr: DEFAULT_CRON_SCHEDULE },
  { label: "Hourly", expr: "0 * * * *" },
  { label: "Daily 05:00", expr: "0 5 * * *" },
] as const;

export function cronHuman(expr: string): string {
  const descriptions: Record<string, string> = {
    [DEFAULT_CRON_SCHEDULE]: "Every 5 minutes.",
    "0 * * * *": "Every hour, on the hour.",
    "0 5 * * *": "Every day at 05:00.",
  };
  return descriptions[expr] ?? "Custom schedule expression.";
}
