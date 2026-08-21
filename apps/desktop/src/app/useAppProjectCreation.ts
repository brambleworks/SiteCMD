import { useCallback, useState } from "react";

import type { ProjectRecord } from "@/hooks/useProject";
import { writeOnboardingSetupSteps } from "@/lib/onboarding-setup";

interface AppProjectCreationOptions {
  refreshProjects: () => Promise<{
    projects: ProjectRecord[];
    newProject: ProjectRecord | null;
  }>;
  queueBaselineScan: (projectId: number) => void;
  selectProject: (project: ProjectRecord) => void;
}

export function useAppProjectCreation({
  refreshProjects,
  queueBaselineScan,
  selectProject,
}: AppProjectCreationOptions) {
  const [showAddProject, setShowAddProject] = useState(false);

  const openAddProject = useCallback(() => {
    setShowAddProject(true);
  }, []);

  const closeAddProject = useCallback(() => {
    setShowAddProject(false);
  }, []);

  const handleProjectCreated = useCallback(
    async (createdProjectId: number) => {
      closeAddProject();
      const { projects: updated } = await refreshProjects();
      if (updated.length > 0) {
        const newProject = updated.find((project) => project.id === createdProjectId) ?? updated[0];
        selectProject(newProject);
        // Queue the baseline until the new project becomes the active selection.
        writeOnboardingSetupSteps(newProject.id, ["baseline-review"]);
        queueBaselineScan(newProject.id);
      }
    },
    [closeAddProject, queueBaselineScan, refreshProjects, selectProject],
  );

  return {
    closeAddProject,
    handleProjectCreated,
    openAddProject,
    showAddProject,
  };
}
