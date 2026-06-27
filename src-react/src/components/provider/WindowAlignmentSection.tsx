import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/primitives";
import { Chip } from "@/components/ui/segmented";
import { Select } from "@/components/ui/select";
import {
  WINDOW_ALIGN_CRON_PRESETS,
  isWindowAlignActive,
  windowAlignCronHuman,
  windowAlignLastAttemptLabel,
  windowAlignStatusLabel,
} from "@/components/provider/providerSchedule";
import type { ProviderScheduleSettings, ProviderTriggerModel } from "@/lib/api/providers";

export interface WindowAlignmentSectionProps {
  providerName: string;
  /** Whether the back-end implements window alignment for this provider. */
  supported: boolean;
  cron: string;
  onCronChange: (cron: string) => void;
  modelId: string | null;
  onModelChange: (modelId: string | null) => void;
  modelOptions: ProviderTriggerModel[];
  modelsLoading: boolean;
  schedule: ProviderScheduleSettings | undefined;
  triggering: boolean;
  quotaFetching: boolean;
  onTriggerNow: () => void;
}

/**
 * Window-alignment schedule editor: cron + trigger model + manual trigger.
 * This is schedule (a billable trigger cadence), not connection params — kept
 * as its own section so the two concerns never re-tangle. Presentational: the
 * cron/model values stay owned by ProviderPage so the footer Save can persist
 * them together with the quota-refresh cadence in one schedule mutation.
 */
export function WindowAlignmentSection({
  providerName,
  supported,
  cron,
  onCronChange,
  modelId,
  onModelChange,
  modelOptions,
  modelsLoading,
  schedule,
  triggering,
  quotaFetching,
  onTriggerNow,
}: WindowAlignmentSectionProps) {
  return (
    <div>
      <div className="mb-1 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
        Window alignment
      </div>
      <div className="mb-3 text-[11px] leading-[1.5] text-[#a99a89]">
        Fire a minimal request at set times so the rolling quota window resets on your schedule.
        Set both a time and a model to turn it on — this makes a real, billable call.
      </div>
      {!supported ? (
        <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px] text-[12px] leading-[1.5] text-[#8a7a68]">
          Coming soon for {providerName} — window alignment is not implemented for this provider
          yet.
        </div>
      ) : (
        <div className="flex flex-col gap-[13px]">
          <div>
            <Input
              className="font-mono"
              placeholder="0 5,10,15,20 * * *"
              value={cron}
              onChange={(e) => onCronChange(e.target.value)}
            />
            <div className="mt-2 flex flex-wrap gap-1.5">
              {WINDOW_ALIGN_CRON_PRESETS.map((preset) => (
                <Chip
                  key={preset.expr}
                  mono
                  active={cron === preset.expr}
                  onClick={() => onCronChange(preset.expr)}
                >
                  {preset.expr}
                </Chip>
              ))}
            </div>
            <div className="mt-2.5 text-[11px] text-[#b3a999]">{windowAlignCronHuman(cron)}</div>
          </div>
          <div className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Trigger model</div>
            <Select
              value={modelId ?? ""}
              onChange={(value) => onModelChange(value || null)}
              options={modelOptions.map((model) => ({
                value: model.id,
                label:
                  model.displayName === model.id
                    ? model.id
                    : `${model.displayName} · ${model.id}`,
              }))}
              placeholder={modelsLoading ? "Loading models…" : "Select a model"}
              disabled={modelsLoading}
            />
            <div className="mt-[5px] text-[11px] text-[#b3a999]">
              {isWindowAlignActive(cron, modelId)
                ? "Active — alignment fires on the schedule above."
                : "Inactive — set both a time and a model to enable."}
            </div>
          </div>
          <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px]">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="text-[12px] font-semibold text-[#6a6055]">Manual trigger</div>
                <div className="mt-[5px] text-[11px] text-[#b3a999]">
                  Last:{" "}
                  <span className="font-medium text-[#7a6f60]">
                    {windowAlignLastAttemptLabel(schedule?.windowAlignLastAttemptAt)}
                  </span>{" "}
                  ·{" "}
                  <span className="font-medium text-[#7a6f60]">
                    {windowAlignStatusLabel(schedule?.windowAlignLastStatus)}
                  </span>
                </div>
                {schedule?.windowAlignLastError ? (
                  <div className="mt-[5px] overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-[#b75548]">
                    {schedule.windowAlignLastError}
                  </div>
                ) : null}
              </div>
              <Button
                variant="subtle"
                size="sm"
                className="flex-none px-3"
                disabled={triggering || quotaFetching || modelsLoading || !modelId?.trim()}
                onClick={onTriggerNow}
              >
                {triggering ? "Triggering..." : "Trigger now"}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
