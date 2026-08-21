const ISSUE_CONFIDENCES = ["confirmed", "high", "needs_review"] as const;
export type IssueConfidence = (typeof ISSUE_CONFIDENCES)[number];

const ISSUE_CONFIDENCE_LABEL: Record<IssueConfidence, string> = {
  confirmed: "Confirmed",
  high: "High confidence",
  needs_review: "Needs review",
};

export const ISSUE_CONFIDENCE_MULTIPLIER: Record<IssueConfidence, number> = {
  confirmed: 1,
  high: 0.85,
  needs_review: 0.55,
};

function normalizeIssueConfidence(value: unknown): IssueConfidence {
  if ((ISSUE_CONFIDENCES as readonly unknown[]).includes(value)) return value as IssueConfidence;
  return "high";
}

export function getIssueConfidence(issue: {
  confidence?: IssueConfidence | string | null;
}): IssueConfidence {
  return normalizeIssueConfidence(issue.confidence);
}

export function getIssueConfidenceLabel(confidence: IssueConfidence): string {
  return ISSUE_CONFIDENCE_LABEL[confidence];
}
