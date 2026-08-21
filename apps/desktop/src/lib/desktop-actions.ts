import {
  openPathInEditor as openPathInEditorCmd,
  revealPath as revealPathCmd,
  runProjectCommand as runProjectCommandCmd,
} from "@/lib/commands";

const PROJECT_COMMAND_CANCELLED = "Project command cancelled";

export interface DesktopCommandResult {
  exitCode: number | null;
  stdout: string;
  stderr: string;
  success: boolean;
}

class ProjectCommandCancelledError extends Error {
  constructor() {
    super(PROJECT_COMMAND_CANCELLED);
    this.name = "ProjectCommandCancelledError";
  }
}

export function extractDesktopCommands(_text: string | null | undefined): string[] {
  // Never derive executable shell commands from untrusted scan evidence.
  return [];
}

export async function runProjectCommand(
  projectPath: string,
  command: string,
): Promise<DesktopCommandResult> {
  try {
    return await runProjectCommandCmd({ projectPath, command });
  } catch (error) {
    if (String(error).includes(PROJECT_COMMAND_CANCELLED)) {
      throw new ProjectCommandCancelledError();
    }
    throw error;
  }
}

export function isProjectCommandCancelled(error: unknown): boolean {
  return (
    error instanceof ProjectCommandCancelledError ||
    (error instanceof Error && error.message === PROJECT_COMMAND_CANCELLED)
  );
}

export async function openPathInEditor(path: string): Promise<void> {
  await openPathInEditorCmd({ path });
}

export async function revealPath(path: string): Promise<void> {
  await revealPathCmd({ path });
}
