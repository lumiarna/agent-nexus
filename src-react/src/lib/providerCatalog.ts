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
