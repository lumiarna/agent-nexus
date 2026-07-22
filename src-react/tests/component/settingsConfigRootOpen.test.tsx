import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const { toast } = vi.hoisted(() => ({ toast: vi.fn() }));
vi.mock("sonner", () => ({ toast }));

import { SettingsPage } from "@/components/settings/SettingsPage";
import { fallbackAgentCapabilities } from "@/lib/agentCapabilities";
import { agentCapabilitiesApi } from "@/lib/api/agentCapabilities";
import { agentCapabilityKeys } from "@/lib/query/agentCapabilities";
import { agentPreferenceKeys } from "@/lib/query/agentPreferences";
import { syncKeys } from "@/lib/query/sync";
import { NavContext } from "@/lib/nav";

function renderSettings() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  queryClient.setQueryData(agentCapabilityKeys.all, fallbackAgentCapabilities());
  queryClient.setQueryData(agentPreferenceKeys.disabled, { disabled: [] });
  queryClient.setQueryData(syncKeys.webdavSettings, {
    url: "",
    user: "",
    pass: "",
    remoteRoot: "",
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <NavContext.Provider value={{ view: "settings", go: () => {} }}>
        <SettingsPage />
      </NavContext.Provider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  toast.mockReset();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe("Settings config-root directory opening", () => {
  it("sends the canonical Agent name through the typed directory-opening API", async () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    const openConfigRoot = vi
      .spyOn(agentCapabilitiesApi, "openConfigRoot")
      .mockResolvedValue(undefined);
    renderSettings();

    const configRoot = screen.getByTitle("Open Generic Agent config root in file manager");
    expect(configRoot.textContent).toBe("~/.agents");
    fireEvent.click(configRoot);

    await waitFor(() =>
      expect(openConfigRoot).toHaveBeenCalledWith("Generic Agent"),
    );
  });

  it("reports the standard desktop-runtime error in browser preview", async () => {
    renderSettings();

    fireEvent.click(screen.getByTitle("Open Generic Agent config root in file manager"));

    await waitFor(() =>
      expect(toast).toHaveBeenCalledWith(
        "Agent Nexus desktop runtime is required for this action.",
      ),
    );
  });
});
