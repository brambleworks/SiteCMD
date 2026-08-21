import { command } from "./invoke";
import type { CreateFixAttemptArgs, FixAttemptDto } from "@/generated/ipc-bindings";

export function createFixAttempt(args: { args: CreateFixAttemptArgs }): Promise<FixAttemptDto> {
  return command<FixAttemptDto>("create_fix_attempt", args);
}

export function getFixAttemptForIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
  title: string;
}): Promise<FixAttemptDto | null> {
  return command<FixAttemptDto | null>("get_fix_attempt_for_issue", args);
}

export function cancelFixAttempt(args: { attemptId: number }): Promise<void> {
  return command<void>("cancel_fix_attempt", args);
}
