import { command } from "./invoke";
import type { DetectedUrl, GitStatus, ProjectInfo, ProjectRecord } from "@/generated/ipc-bindings";

export function detectProjectUrls(args: { path: string }): Promise<ProjectInfo> {
  return command<ProjectInfo>("detect_project_urls", args);
}

export function addProject(args: {
  name: string;
  path: string;
  framework?: string | null;
  urls: DetectedUrl[];
}): Promise<number> {
  return command<number>("add_project", args);
}

export function addProjectByUrl(args: { name: string; url: string }): Promise<number> {
  return command<number>("add_project_by_url", args);
}

export function renameProject(args: { projectId: number; name: string }): Promise<void> {
  return command<void>("rename_project", args);
}

export function updateProjectPath(args: { projectId: number; path: string }): Promise<void> {
  return command<void>("update_project_path", args);
}

export function getProjects(): Promise<ProjectRecord[]> {
  return command<ProjectRecord[]>("get_projects");
}

export function deleteProject(args: { projectId: number }): Promise<void> {
  return command<void>("delete_project", args);
}

export function addEnvironmentUrl(args: {
  projectId: number;
  url: string;
  label: string;
  environment: string;
}): Promise<number> {
  return command<number>("add_environment_url", args);
}

export function deleteEnvironment(args: { environmentId: number }): Promise<void> {
  return command<void>("delete_environment", args);
}

export function getGitStatus(args: { projectId: number; limit?: number }): Promise<GitStatus> {
  return command<GitStatus>("get_git_status", args);
}
