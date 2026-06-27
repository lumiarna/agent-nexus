import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ApiKeyForm } from "@/components/provider/connection/ApiKeyForm";
import { CopilotTokenForm } from "@/components/provider/connection/CopilotTokenForm";
import { OpenCodeGoConnectionForm } from "@/components/provider/connection/OpenCodeGoConnectionForm";

// Mark the runtime as desktop so the forms' on-mount load runs. The connection
// forms themselves import no Tauri APIs — load/onSave are injected — so these
// tests exercise the full "load → 填写 → 保存 → saving 态" path with fakes.
beforeEach(() => {
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
});

afterEach(() => {
  cleanup();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe("CopilotTokenForm", () => {
  it("loads the existing token, forwards edits to onSave, and reflects saving state", async () => {
    let resolveSave: (() => void) | undefined;
    const onSave = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const load = vi.fn().mockResolvedValue("gho_existing");

    render(<CopilotTokenForm load={load} onSave={onSave} />);

    const input = await screen.findByPlaceholderText<HTMLInputElement>("gho_... or ghp_...");
    await waitFor(() => expect(input.value).toBe("gho_existing"));

    fireEvent.change(input, { target: { value: "gho_new" } });
    fireEvent.click(screen.getByRole("button", { name: "Save connection" }));

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("gho_new");

    // While onSave is pending the button shows the saving state and is disabled.
    const saving = await screen.findByRole("button", { name: "Saving..." });
    expect(saving).toHaveProperty("disabled", true);

    resolveSave?.();
    await screen.findByRole("button", { name: "Save connection" });
  });
});

describe("OpenCodeGoConnectionForm", () => {
  it("loads and forwards both workspace id and auth cookie to onSave", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const load = vi.fn().mockResolvedValue({ workspaceId: "wrk_1", authCookie: "ck_1" });

    render(<OpenCodeGoConnectionForm load={load} onSave={onSave} />);

    const workspace = await screen.findByPlaceholderText<HTMLInputElement>("wrk_xxxxxxxxxxxx");
    await waitFor(() => expect(workspace.value).toBe("wrk_1"));
    const cookie = screen.getByPlaceholderText("Fe26.2**...");

    fireEvent.change(workspace, { target: { value: "wrk_2" } });
    fireEvent.change(cookie, { target: { value: "ck_2" } });
    fireEvent.click(screen.getByRole("button", { name: "Save connection" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({ workspaceId: "wrk_2", authCookie: "ck_2" }),
    );
  });
});

describe("ApiKeyForm", () => {
  it("loads and forwards the API key to onSave using the provided hint placeholder", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const load = vi.fn().mockResolvedValue("sk-existing");

    render(
      <ApiKeyForm
        hint={{ placeholder: "sk-test", authKey: "deepseek", savedLabel: "DeepSeek API key" }}
        load={load}
        onSave={onSave}
      />,
    );

    const input = await screen.findByPlaceholderText<HTMLInputElement>("sk-test");
    await waitFor(() => expect(input.value).toBe("sk-existing"));

    fireEvent.change(input, { target: { value: "sk-new" } });
    fireEvent.click(screen.getByRole("button", { name: "Save connection" }));

    await waitFor(() => expect(onSave).toHaveBeenCalledWith("sk-new"));
  });

  it("skips load outside the desktop runtime", async () => {
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    const onSave = vi.fn().mockResolvedValue(undefined);
    const load = vi.fn().mockResolvedValue("sk-existing");

    render(
      <ApiKeyForm
        hint={{ placeholder: "sk-test", authKey: "deepseek", savedLabel: "DeepSeek API key" }}
        load={load}
        onSave={onSave}
      />,
    );

    await screen.findByPlaceholderText("sk-test");
    expect(load).not.toHaveBeenCalled();
  });
});
