function normalizeDossierCopy(value: string | null | undefined): string {
  return (
    value
      ?.toLowerCase()
      .replace(/https?:\/\/\S+/g, "")
      .replace(/[^\p{L}\p{N}\s]/gu, " ")
      .replace(/\s+/g, " ")
      .trim() ?? ""
  );
}

export function isDuplicateDossierCopy(
  candidate: string | null | undefined,
  reference: string | null | undefined,
): boolean {
  const candidateText = normalizeDossierCopy(candidate);
  const referenceText = normalizeDossierCopy(reference);
  if (!candidateText || !referenceText) return false;
  return candidateText === referenceText;
}

export function pickSupportingDossierCopy(
  reference: string | null | undefined,
  candidates: Array<string | null | undefined>,
): string | null {
  const seen = new Set([normalizeDossierCopy(reference)]);

  for (const candidate of candidates) {
    const trimmed = candidate?.trim();
    const normalized = normalizeDossierCopy(trimmed);
    if (!trimmed || !normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    return trimmed;
  }

  return null;
}
