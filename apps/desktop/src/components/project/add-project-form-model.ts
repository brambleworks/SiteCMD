import type { ProjectEnvironment } from "@/lib/project-environments";
import type { NavTarget } from "@/components/layout/nav-page";

export interface AddProjectFormProps {
  onCreated: (projectId: number) => void;
  onCancel: () => void;
  onNavigate?: (page: NavTarget) => void;
}

export type UrlRow = { url: string; environment: ProjectEnvironment };

export function buildInitialUrls(): UrlRow[] {
  return [{ url: "", environment: "production" }];
}
