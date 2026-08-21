export function DossierConfidenceRow({ label, reason }: { label: string; reason?: string | null }) {
  const normalizedReason = reason?.trim() || null;

  return (
    <div className="dossier-rail-row">
      <span className="dossier-rail-row-key">Confidence</span>
      <div className="dossier-rail-body">
        <span className="dossier-rail-row-value">{label}</span>
        {normalizedReason ? <p className="dossier-confidence-reason">{normalizedReason}</p> : null}
      </div>
    </div>
  );
}
