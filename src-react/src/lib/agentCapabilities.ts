import { AGENTS } from "@/config/agents";
import type { AgentCapabilitySurface } from "@/lib/api/agentCapabilities";
import type { Provider } from "@/types";

export function fallbackAgentCapabilities(): AgentCapabilitySurface[] {
  return AGENTS.map((agent) => {
    const configDir = agent.dirs[0]?.value ?? "";
    const globalSkillDir = agent.dirs.find((dir) => dir.key.endsWith("_SKILLS_DIR"))?.value;
    const globalPromptFile = agent.dirs.find((dir) => dir.key.endsWith("_PROMPT_FILE"))?.value;

    return {
      name: agent.name,
      abbr: agent.abbr,
      color: agent.color,
      configDir,
      skill:
        globalSkillDir && agent.projectSkillDir
          ? {
              globalDir: globalSkillDir,
              projectDir: agent.projectSkillDir,
            }
          : null,
      prompt: globalPromptFile
        ? {
            globalFile: globalPromptFile,
            projectFile: agent.projectPromptFile ?? null,
          }
        : null,
      provider: agent.providerId
        ? {
            providerId: agent.providerId,
            credentialHint: agent.authFile ?? null,
          }
        : null,
    };
  });
}

export function providerRowsFromAgentCapabilities(
  capabilities: readonly AgentCapabilitySurface[],
  existingProviders: readonly Provider[],
): Provider[] {
  const existingById = new Map(existingProviders.map((provider) => [provider.id, provider]));
  const agentProviderIds = new Set<string>();
  const agentProviders = capabilities.flatMap((agent) => {
    if (!agent.provider) return [];

    const existing = existingById.get(agent.provider.providerId);
    agentProviderIds.add(agent.provider.providerId);

    return [
      {
        ...existing,
        id: agent.provider.providerId,
        name: agent.name,
        plan: existing?.plan ?? "—",
        status: existing?.status ?? "nocreds",
        credential: agent.provider.credentialHint ?? existing?.credential ?? "not found",
        isAgent: true,
      } satisfies Provider,
    ];
  });

  const nonAgentProviders = existingProviders.filter(
    (provider) => !provider.isAgent && !agentProviderIds.has(provider.id),
  );

  return [...agentProviders, ...nonAgentProviders];
}

export function reconcileProviderRows(
  current: readonly Provider[],
  catalog: readonly Provider[],
): Provider[] {
  const currentById = new Map(current.map((provider) => [provider.id, provider]));

  return catalog.map((provider) => {
    const existing = currentById.get(provider.id);
    if (!existing) return provider;

    return {
      ...provider,
      ...existing,
      name: provider.name,
      isAgent: provider.isAgent,
      needsParams: provider.needsParams,
      hiddenCard: provider.hiddenCard,
      credential: existing.credential || provider.credential,
    };
  });
}
