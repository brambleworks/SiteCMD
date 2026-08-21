export interface ProjectCapabilities {
  hasSite: boolean;
  hasCode: boolean;
}

export function getProjectCapabilities(input: {
  environmentUrl?: string | null;
  projectFolder?: string | null;
}): ProjectCapabilities {
  return {
    hasSite: Boolean(input.environmentUrl?.trim()),
    hasCode: Boolean(input.projectFolder?.trim()),
  };
}

// Match the backend scope for code-only project findings.
export const NO_SITE_SCOPE_URL = "";
