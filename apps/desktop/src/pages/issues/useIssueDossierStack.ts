import { useCallback, useState } from "react";
import type { UnifiedFixIssue } from "@/components/issues/IssueList";
import { findUnifiedByCheckId } from "@/lib/issue-ranking";

export function useIssueDossierStack(rankedIssues: UnifiedFixIssue[], onMissingCause: () => void) {
  const [selectedStack, setSelectedStack] = useState<UnifiedFixIssue[]>([]);
  const selectedIssue = selectedStack.length > 0 ? selectedStack[selectedStack.length - 1] : null;

  const selectIssue = useCallback((item: UnifiedFixIssue) => {
    setSelectedStack([item]);
  }, []);

  const closeIssue = useCallback(() => {
    setSelectedStack([]);
  }, []);

  const goBack = useCallback(() => {
    setSelectedStack((prev) => (prev.length > 1 ? prev.slice(0, -1) : prev));
  }, []);

  const openCause = useCallback(
    (checkId: string) => {
      const match = findUnifiedByCheckId(rankedIssues, checkId);
      if (!match) {
        onMissingCause();
        return;
      }
      setSelectedStack((prev) => [...prev, match].slice(-5));
    },
    [onMissingCause, rankedIssues],
  );

  const resetIssueStack = useCallback(() => {
    setSelectedStack([]);
  }, []);

  return {
    selectedStack,
    selectedIssue,
    selectIssue,
    closeIssue,
    goBack,
    openCause,
    resetIssueStack,
  };
}
