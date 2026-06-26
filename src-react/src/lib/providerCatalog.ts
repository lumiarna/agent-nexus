import type { Provider } from "../types/index.js";

const BUILT_IN_PROVIDERS = [
  {
    id: "opencode-go",
    name: "OpenCode Go",
    plan: "Workspace",
    status: "nocreds",
    credential: "manual · workspace id + cookie",
    needsParams: true,
  },
  {
    id: "minimax-token",
    name: "MiniMax Token Plan CN",
    plan: "Token plan",
    status: "nocreds",
    credential: "manual API key or opencode auth.json · minimax-cn-coding-plan",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    plan: "Balance",
    status: "nocreds",
    credential: "manual API key or opencode auth.json · deepseek",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    plan: "Credits",
    status: "nocreds",
    credential: "manual API key or opencode auth.json · openrouter",
  },
] satisfies Provider[];

interface CustomProviderCatalogEntry {
  id: string;
  name: string;
  npm: string;
  baseUrl: string;
  modelId: string;
}

interface ExistingProviderCatalogEntry {
  id: string;
}

interface CustomProviderRow {
  id: string;
  name: string;
  plan: string;
  status: "nocreds";
  credential: string;
}

export function builtInProviderRows(): Provider[] {
  return BUILT_IN_PROVIDERS.map((provider) => ({ ...provider }));
}

export function customProviderRows<TExisting extends ExistingProviderCatalogEntry>(
  customProviders: readonly CustomProviderCatalogEntry[],
  existingProviders: readonly TExisting[],
): CustomProviderRow[] {
  const existingIds = new Set(existingProviders.map((provider) => provider.id));
  return customProviders
    .filter((provider) => !existingIds.has(provider.id))
    .map((provider) => ({
      id: provider.id,
      name: provider.name,
      plan: "OpenCode custom",
      status: "nocreds",
      credential: `opencode.json · ${provider.id}`,
    }));
}
