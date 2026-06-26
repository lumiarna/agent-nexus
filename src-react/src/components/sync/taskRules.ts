// Pure validity rules for the Sync Task forms, shared by the "create custom task" and
// "add task" surfaces so the rules live in one place and can be unit-tested. The backend
// `prepare_task` stays the transactional source of truth; these mirror it for UX only
// (which action options are disabled, how schedule couples to action). Kept dependency-free
// so it compiles under the NodeNext test build, like the other tested pure modules.

export type LocationType = "Local" | "Cloud";
export type TaskAction = "Symlink" | "Junction" | "Copy";

export interface TaskEndpoints {
  sourceType: LocationType;
  targets: { type: LocationType }[];
}

export interface ActionOption {
  value: TaskAction;
  label: string;
  disabled?: boolean;
}

/** Symlink/Junction require a Local→Local task, so any Cloud endpoint disables both. */
export function hasCloudEndpoint(task: TaskEndpoints): boolean {
  return task.sourceType === "Cloud" || task.targets.some((target) => target.type === "Cloud");
}

/** Action picker options: Symlink always shown, Junction only on Windows, Copy always enabled.
 *  Symlink and Junction are disabled while either endpoint is Cloud. */
export function actionOptions(task: TaskEndpoints, supportsJunction: boolean): ActionOption[] {
  const hasCloud = hasCloudEndpoint(task);
  return [
    { value: "Symlink", label: "Symlink", disabled: hasCloud },
    ...(supportsJunction
      ? [{ value: "Junction" as TaskAction, label: "Junction", disabled: hasCloud }]
      : []),
    { value: "Copy", label: "Copy" },
  ];
}

/** A schedule only applies to a Copy task; any other action forces "manual". */
export function scheduleForAction(action: TaskAction, currentSchedule: string): string {
  return action === "Copy" ? currentSchedule : "manual";
}

/** Whether a schedule string is a cron expression (anything other than "manual"). */
export function isCronSchedule(schedule: string): boolean {
  return schedule !== "manual";
}

/** Toggle a schedule between manual and cron, keeping an existing cron when re-enabling and
 *  applying `defaultCron` when there is none yet. */
export function scheduleForMode(
  mode: "manual" | "cron",
  currentSchedule: string,
  defaultCron: string,
): string {
  if (mode === "manual") {
    return "manual";
  }
  return isCronSchedule(currentSchedule) ? currentSchedule : defaultCron;
}

/** Collapse blank/whitespace schedules to the canonical "manual". */
export function normalizeSchedule(schedule: string): string {
  return (schedule || "manual").trim() || "manual";
}

export interface FormTarget {
  type: LocationType;
  path: string;
}

export interface FormTask {
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targets: FormTarget[];
  schedule: string;
}

export interface DraftTaskInput {
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targetType: LocationType;
  target: string;
  schedule: string;
}

/** Expand a form task (1 source → N targets) into the domain's single-target task inputs,
 *  dropping targets whose path is blank. Mirrors the "1 source → 1 target" Sync Task model. */
export function expandFormTask(task: FormTask): DraftTaskInput[] {
  return task.targets
    .filter((target) => target.path.trim())
    .map((target) => ({
      action: task.action,
      sourceType: task.sourceType,
      source: task.source,
      targetType: target.type,
      target: target.path.trim(),
      schedule: normalizeSchedule(task.schedule),
    }));
}
