import { CODE_SCAN_DOMAIN_META } from "@/lib/code-scan-domains";
import { SCAN_LABELS } from "@/lib/scan-labels";
import type { CodeScanDomain } from "@/lib/types";

interface WorkflowCopyCue {
  label: string;
  sentence: string;
}

interface CodeScanCompletionCopyInput {
  score: number;
  issueCount: number;
  scoreMessage: string;
  host?: string | null;
  titleLabel?: string;
  jobLabel?: string;
  leadingDomain?: {
    label: string;
    shortLabel: string;
    count: number;
  } | null;
  domainTrendLabel?: string | null;
  workflowCue?: WorkflowCopyCue | null;
  previousScore?: number | null;
  resolvedCount?: number | null;
}

interface WebScanCompletionCopyInput {
  score: number;
  issueCount: number;
  scoreMessage: string;
  host?: string | null;
  titleLabel?: string;
  jobLabel?: string;
  workflowCue?: WorkflowCopyCue | null;
  previousScore?: number | null;
  resolvedCount?: number | null;
}

interface MultiScanCompletionCopyInput {
  score: number;
  pageCount: number;
  scoreMessage: string;
  titleLabel?: string;
  jobLabel?: string;
  workflowCue?: WorkflowCopyCue | null;
}

interface ScheduledScanCompletionCopyInput {
  scanType?: string | null;
  score: number;
  issueCount: number;
  host: string;
  scoreMessage: string;
  topDomain?: CodeScanDomain | null;
  topDomainCount?: number | null;
  domainTrendLabel?: string | null;
  workflowCue?: WorkflowCopyCue | null;
}

interface CompletionCopy {
  title: string;
  body: string;
  jobLabel: string;
  jobDetail: string;
}

function joinDetail(parts: Array<string | null | undefined | false>): string {
  return parts
    .filter((part): part is string => typeof part === "string" && part.trim().length > 0)
    .join(" • ");
}

function buildProgressSentence(
  previousScore: number | null | undefined,
  currentScore: number,
  resolvedCount: number | null | undefined,
): string {
  const parts: string[] = [];
  if (previousScore != null) {
    const delta = currentScore - previousScore;
    if (delta > 0) parts.push(`Score up ${delta} point${delta === 1 ? "" : "s"}`);
    else if (delta < 0)
      parts.push(`Score down ${Math.abs(delta)} point${Math.abs(delta) === 1 ? "" : "s"}`);
  }
  if (resolvedCount != null && resolvedCount > 0) {
    parts.push(`${resolvedCount} issue${resolvedCount === 1 ? "" : "s"} resolved`);
  }
  return parts.length > 0 ? parts.join(", ") + "." : "";
}

function buildProgressTitle(
  previousScore: number | null | undefined,
  currentScore: number,
): string {
  if (previousScore == null) return "";
  const delta = currentScore - previousScore;
  if (delta >= 10) return " (+${delta})".replace("${delta}", String(delta));
  if (delta > 0) return " (+${delta})".replace("${delta}", String(delta));
  return "";
}

function getIssueLabel(count: number): string {
  return `${count} issue${count === 1 ? "" : "s"}`;
}

export function getScheduledScanLabel(scanType?: string | null): string {
  if (scanType === "code") return `Scheduled ${SCAN_LABELS.code}`;
  if (scanType === "full") return `Scheduled ${SCAN_LABELS.full}`;
  return `Scheduled ${SCAN_LABELS.web}`;
}

export function buildCodeScanCompletionCopy(input: CodeScanCompletionCopyInput): CompletionCopy {
  const titleLabel = input.titleLabel ?? SCAN_LABELS.code;
  const jobLabel = input.jobLabel ?? "Code scan";
  const hostSuffix = input.host ? ` for ${input.host}` : "";
  const leadingDomainSentence = input.leadingDomain
    ? ` ${input.leadingDomain.label} leads with ${input.leadingDomain.count}.`
    : "";
  const domainTrendSentence = input.domainTrendLabel ? ` ${input.domainTrendLabel}.` : "";
  const workflowSentence = input.workflowCue ? ` ${input.workflowCue.sentence}` : "";
  const progressSentence = buildProgressSentence(
    input.previousScore,
    input.score,
    input.resolvedCount,
  );
  const titleDelta = buildProgressTitle(input.previousScore, input.score);

  return {
    title: `${titleLabel} Complete - ${input.score}/100${titleDelta}`,
    body: `${progressSentence ? progressSentence + " " : ""}${input.issueCount} code issues found${hostSuffix}.${leadingDomainSentence}${domainTrendSentence} ${input.scoreMessage}${workflowSentence}`.trim(),
    jobLabel,
    jobDetail: joinDetail([
      `${input.score}/100`,
      getIssueLabel(input.issueCount),
      progressSentence || null,
      input.leadingDomain ? `${input.leadingDomain.shortLabel} ${input.leadingDomain.count}` : null,
      input.domainTrendLabel,
      input.workflowCue?.label,
    ]),
  };
}

export function buildWebScanCompletionCopy(input: WebScanCompletionCopyInput): CompletionCopy {
  const titleLabel = input.titleLabel ?? SCAN_LABELS.web;
  const jobLabel = input.jobLabel ?? "Web scan";
  const location = input.host ? ` on ${input.host}` : "";
  const workflowSentence = input.workflowCue ? ` ${input.workflowCue.sentence}` : "";
  const progressSentence = buildProgressSentence(
    input.previousScore,
    input.score,
    input.resolvedCount,
  );
  const titleDelta = buildProgressTitle(input.previousScore, input.score);

  return {
    title: `${titleLabel} Complete - ${input.score}/100${titleDelta}`,
    body: `${progressSentence ? progressSentence + " " : ""}${input.issueCount} issues found${location}. ${input.scoreMessage}${workflowSentence}`.trim(),
    jobLabel,
    jobDetail: joinDetail([
      `${input.score}/100`,
      getIssueLabel(input.issueCount),
      progressSentence || null,
      input.workflowCue?.label,
    ]),
  };
}

export function buildMultiScanCompletionCopy(input: MultiScanCompletionCopyInput): CompletionCopy {
  const titleLabel = input.titleLabel ?? SCAN_LABELS.multiPageWeb;
  const jobLabel = input.jobLabel ?? "Multi-page scan";
  const workflowSentence = input.workflowCue ? ` ${input.workflowCue.sentence}` : "";

  return {
    title: `${titleLabel} Complete - ${input.score}/100`,
    body: `${input.pageCount} pages scanned. ${input.scoreMessage}${workflowSentence}`.trim(),
    jobLabel,
    jobDetail: joinDetail([
      `${input.score}/100`,
      `${input.pageCount} pages`,
      input.workflowCue?.label,
    ]),
  };
}

export function buildScheduledScanCompletionCopy(
  input: ScheduledScanCompletionCopyInput,
): CompletionCopy {
  const label = getScheduledScanLabel(input.scanType);
  if (input.scanType === "code") {
    const topDomainMeta = input.topDomain ? CODE_SCAN_DOMAIN_META[input.topDomain] : null;
    return buildCodeScanCompletionCopy({
      titleLabel: label,
      jobLabel: label,
      score: input.score,
      issueCount: input.issueCount,
      scoreMessage: input.scoreMessage,
      host: input.host,
      leadingDomain:
        topDomainMeta && input.topDomainCount
          ? {
              label: topDomainMeta.label,
              shortLabel: topDomainMeta.shortLabel,
              count: input.topDomainCount,
            }
          : null,
      domainTrendLabel: input.domainTrendLabel,
      workflowCue: input.workflowCue,
    });
  }

  return buildWebScanCompletionCopy({
    titleLabel: label,
    jobLabel: label,
    score: input.score,
    issueCount: input.issueCount,
    scoreMessage: input.scoreMessage,
    host: input.host,
    workflowCue: input.workflowCue,
  });
}
