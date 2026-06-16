import { createContext, useContext } from "react";

/** Desktop app uses state-driven view switching — no router (ADR0001). */
export type View =
  | "provider"
  | "project"
  | "skill"
  | "prompt"
  | "session"
  | "sync"
  | "settings";

export interface Nav {
  view: View;
  /** Switch view. `projectId` deep-links Project straight into its detail. */
  go: (view: View, opts?: { projectId?: string }) => void;
}

export const NavContext = createContext<Nav>({ view: "provider", go: () => {} });

export const useNav = (): Nav => useContext(NavContext);
