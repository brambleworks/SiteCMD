export interface GitCommit {
  hash: string;
  shortHash: string;
  message: string;
  author: string;
  date: string;
  relativeDate: string;
}

export interface GitStatus {
  isGitRepo: boolean;
  branch: string | null;
  commits: GitCommit[];
  totalCommits: number;
  hasUncommitted: boolean;
}

export interface DeployScanSummary {
  id: number;
  url: string;
  mode: string;
  overallScore: number;
  issuesTotal: number;
  issuesCritical: number;
  issuesHigh: number;
  durationMs: number;
  timestamp: string;
}

export interface DeployCorrelation {
  sourceEventId: number;
  targetEventId: number | null;
  correlationType: string;
  confidence: "high" | "medium" | "low";
  description: string;
  sourceTimestamp: string;
  targetTimestamp: string | null;
}

export interface DeploysPageProps {
  projectPath: string | null;
  projectId: number;
  url: string;
  onScan: () => void;
  scanning: boolean;
  onViewScan: (scanId: number) => void;
  onAddFolder: () => void;
}
