export function supportsWindowAlignment(providerId: string | null | undefined): boolean {
  return providerId === "claude" || providerId === "codex";
}
