import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ApiKeyForm } from "@/components/provider/connection/ApiKeyForm";
import { CopilotTokenForm } from "@/components/provider/connection/CopilotTokenForm";
import { OpenCodeGoConnectionForm } from "@/components/provider/connection/OpenCodeGoConnectionForm";

// Mark the runtime as desktop so the forms work. The connection forms themselves
// import no Tauri APIs — value/onChange are injected — so these tests exercise
// the controlled component behavior.
beforeEach(() => {
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
});

afterEach(() => {
  cleanup();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe("CopilotTokenForm", () => {
  it("renders the initial value and forwards onChange edits", async () => {
    const onChange = vi.fn();
    const { rerender } = render(<CopilotTokenForm value="gho_existing" onChange={onChange} />);

    const input = screen.getByPlaceholderText<HTMLInputElement>("gho_... or ghp_...");
    expect(input.value).toBe("gho_existing");

    fireEvent.change(input, { target: { value: "gho_new" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("gho_new");

    // Re-render with the new value (as the parent would)
    rerender(<CopilotTokenForm value="gho_new" onChange={onChange} />);
    expect(input.value).toBe("gho_new");
  });

  it("clears the input when value is empty", () => {
    const onChange = vi.fn();
    render(<CopilotTokenForm value="" onChange={onChange} />);

    const input = screen.getByPlaceholderText<HTMLInputElement>("gho_... or ghp_...");
    expect(input.value).toBe("");
  });
});

describe("OpenCodeGoConnectionForm", () => {
  it("renders workspace id and auth cookie, forwards edits to onChange", () => {
    const onChange = vi.fn();
    const value = { workspaceId: "wrk_1", authCookie: "ck_1" };
    const { rerender } = render(
      <OpenCodeGoConnectionForm value={value} onChange={onChange} />,
    );

    const workspace = screen.getByPlaceholderText<HTMLInputElement>("wrk_xxxxxxxxxxxx");
    expect(workspace.value).toBe("wrk_1");

    const cookie = screen.getByPlaceholderText<HTMLInputElement>("Fe26.2**...");
    expect(cookie.value).toBe("ck_1");

    // Change workspace id
    fireEvent.change(workspace, { target: { value: "wrk_2" } });
    expect(onChange).toHaveBeenCalledWith({ workspaceId: "wrk_2", authCookie: "ck_1" });

    // Change auth cookie
    onChange.mockClear();
    fireEvent.change(cookie, { target: { value: "ck_2" } });
    expect(onChange).toHaveBeenCalledWith({ workspaceId: "wrk_1", authCookie: "ck_2" });

    // Re-render with updated value
    rerender(
      <OpenCodeGoConnectionForm
        value={{ workspaceId: "wrk_2", authCookie: "ck_2" }}
        onChange={onChange}
      />,
    );
    expect(workspace.value).toBe("wrk_2");
    expect(cookie.value).toBe("ck_2");
  });

  it("handles empty value gracefully", () => {
    const onChange = vi.fn();
    const value = { workspaceId: "", authCookie: "" };
    render(<OpenCodeGoConnectionForm value={value} onChange={onChange} />);

    const workspace = screen.getByPlaceholderText<HTMLInputElement>("wrk_xxxxxxxxxxxx");
    expect(workspace.value).toBe("");

    const cookie = screen.getByPlaceholderText<HTMLInputElement>("Fe26.2**...");
    expect(cookie.value).toBe("");
  });
});

describe("ApiKeyForm", () => {
  it("renders the initial value with the given hint placeholder and forwards onChange", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <ApiKeyForm
        hint={{ placeholder: "sk-test", authKey: "deepseek", savedLabel: "DeepSeek API key" }}
        value="sk-existing"
        onChange={onChange}
      />,
    );

    const input = screen.getByPlaceholderText<HTMLInputElement>("sk-test");
    expect(input.value).toBe("sk-existing");

    fireEvent.change(input, { target: { value: "sk-new" } });
    expect(onChange).toHaveBeenCalledWith("sk-new");

    rerender(
      <ApiKeyForm
        hint={{ placeholder: "sk-test", authKey: "deepseek", savedLabel: "DeepSeek API key" }}
        value="sk-new"
        onChange={onChange}
      />,
    );
    expect(input.value).toBe("sk-new");
  });

  it("shows empty value when no initial value is set", () => {
    const onChange = vi.fn();
    render(
      <ApiKeyForm
        hint={{ placeholder: "sk-test", authKey: "deepseek", savedLabel: "DeepSeek API key" }}
        value=""
        onChange={onChange}
      />,
    );

    const input = screen.getByPlaceholderText<HTMLInputElement>("sk-test");
    expect(input.value).toBe("");
  });
});
