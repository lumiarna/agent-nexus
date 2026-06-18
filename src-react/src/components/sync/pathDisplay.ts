function stripWindowsVerbatimPrefix(path: string): string {
  const windowsUncPrefix = "\\\\?\\UNC\\";
  const windowsPrefix = "\\\\?\\";

  if (path.startsWith(windowsUncPrefix)) {
    return "\\\\" + path.slice(windowsUncPrefix.length);
  }
  if (path.startsWith(windowsPrefix)) {
    return path.slice(windowsPrefix.length);
  }
  if (path.startsWith("//?/UNC/")) {
    return `//${path.slice("//?/UNC/".length)}`;
  }
  if (path.startsWith("//?/")) {
    return path.slice("//?/".length);
  }
  return path;
}

/** Show registered Project paths as project-relative paths while preserving external absolute paths. */
export function formatProjectSymlinkDisplayPath(
  fullPath: string,
  projectName?: string | null,
): string {
  const displayPath = stripWindowsVerbatimPrefix(fullPath);
  const project = projectName?.trim();
  if (!project) return displayPath;

  const normalizedSegments = displayPath.replace(/\\/g, "/").split("/");
  const projectIndex = normalizedSegments.findIndex((segment) => segment === project);
  if (projectIndex === -1) return displayPath;

  return normalizedSegments.slice(projectIndex + 1).join("/") || "/";
}
