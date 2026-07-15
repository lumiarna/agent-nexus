import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import { SkillRow } from "@/components/skill/SkillRow";
import type { PlacementCells, Skill } from "@/types";

const cells: PlacementCells = {
  "Generic Agent": "none",
  "Claude Code": "none",
  CodeX: "none",
  Copilot: "none",
  OpenCode: "none",
  Pi: "none",
  Qoder: "none",
};
const summary = {
  skillId: "skill-1",
  name: "Demo",
  desc: "Demo Skill",
  path: "/demo",
  disabled: false,
};

function props(skill: Skill, onIntent = vi.fn()) {
  return {
    skill,
    mode: "project" as const,
    onToggleAgentCell: vi.fn(),
    onProjectCustomIntent: onIntent,
    onToggleDmi: vi.fn(),
    onOpen: vi.fn(),
    onReveal: vi.fn(),
  };
}

afterEach(cleanup);

describe("SkillRow Project custom intent delegation", () => {
  it("source Add emits one SetTargetEnabled intent without a default Agent", () => {
    const onIntent = vi.fn();
    const skill: Skill = {
      kind: "projectCustomCanonical",
      rowKey: "skill-1",
      skill: summary,
      sourceProject: { id: "source", name: "Source" },
      destinations: [
        { kind: "global", cells },
        {
          kind: "project",
          project: { id: "target", name: "Target" },
          cells,
        },
      ],
    };
    render(<SkillRow {...props(skill, onIntent)} />);

    fireEvent.click(screen.getByRole("button", { name: /Propagate to/ }));
    const addButtons = screen.getAllByRole("button", { name: /Add/ });
    fireEvent.click(addButtons[1]);

    expect(onIntent).toHaveBeenCalledTimes(1);
    expect(onIntent).toHaveBeenCalledWith({
      kind: "setTargetEnabled",
      skillId: "skill-1",
      destination: { kind: "project", projectId: "target" },
      enabled: true,
    });
    expect(onIntent.mock.calls[0][0]).not.toHaveProperty("defaultAgent");
  });

  it("incoming Agent cell emits an exact SetAgentPlacement intent", () => {
    const onIntent = vi.fn();
    const skill: Skill = {
      kind: "projectCustomIncoming",
      rowKey: "incoming",
      skill: summary,
      sourceProject: { id: "source", name: "Source" },
      targetProject: { id: "target", name: "Target" },
      cells,
    };
    render(<SkillRow {...props(skill, onIntent)} />);

    fireEvent.click(screen.getByTitle(/CodeX · no placement/));

    expect(onIntent).toHaveBeenCalledTimes(1);
    expect(onIntent).toHaveBeenCalledWith({
      kind: "setAgentPlacement",
      skillId: "skill-1",
      destination: { kind: "project", projectId: "target" },
      agent: "CodeX",
      enabled: true,
    });
  });
});
