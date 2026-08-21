import { command } from "./invoke";
import type {
  FixLocation,
  IssueCheckMemory,
  IssueGroup,
  IssueLink,
  IssueVerificationOutcome,
  PageSummary,
  ScoreSnapshot,
} from "@/generated/ipc-bindings";

// Pin the IssueStateRow tuple until generated bindings cover it.
export type IssueStateRow = [string, number | null, string | null, string | null];

export function getWorkItems(args: {
  projectId: number;
  envUrl?: string | null;
}): Promise<IssueGroup[]> {
  return command<IssueGroup[]>("get_work_items", args);
}

export function getCurrentScore(args: {
  projectId: number;
  envUrl?: string | null;
}): Promise<ScoreSnapshot> {
  return command<ScoreSnapshot>("get_current_score", args);
}

export function getIssueState(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
}): Promise<IssueStateRow | null> {
  return command<IssueStateRow | null>("get_issue_state", args);
}

export function getIssueCheckMemory(args: {
  projectId: number;
  checkId: string;
}): Promise<IssueCheckMemory> {
  return command<IssueCheckMemory>("get_issue_check_memory", args);
}

export function snoozeIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
  snoozeUntil: number;
}): Promise<void> {
  return command<void>("snooze_issue", args);
}

export function ignoreIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
}): Promise<void> {
  return command<void>("ignore_issue", args);
}

export function blockIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
  reason: string;
}): Promise<void> {
  return command<void>("block_issue", args);
}

export function reopenIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
}): Promise<void> {
  return command<void>("reopen_issue", args);
}

export function verifyIssue(args: {
  projectId: number;
  envUrl?: string | null;
  checkId: string;
}): Promise<IssueVerificationOutcome> {
  return command<IssueVerificationOutcome>("verify_issue", args);
}

export function getIssuesForPage(args: {
  projectId: number;
  envUrl: string;
  pageUrl: string;
}): Promise<IssueGroup[]> {
  return command<IssueGroup[]>("get_issues_for_page", args);
}

export function getPagesWithIssues(args: {
  projectId: number;
  envUrl: string;
}): Promise<PageSummary[]> {
  return command<PageSummary[]>("get_pages_with_issues", args);
}

export function resolveFixLocationsForCheck(args: {
  checkId: string;
  projectId: number;
}): Promise<FixLocation[]> {
  return command<FixLocation[]>("resolve_fix_locations_for_check", args);
}

export function createIssueLink(args: {
  projectId: number;
  checkId: string;
  scanId: number;
  provider: string;
  estimatedImpact: number;
}): Promise<IssueLink> {
  return command<IssueLink>("create_issue_link", args);
}

export function getIssueLinkForCheck(args: {
  projectId: number;
  checkId: string;
}): Promise<IssueLink | null> {
  return command<IssueLink | null>("get_issue_link_for_check", args);
}
