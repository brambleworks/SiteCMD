import type { ReactNode } from "react";
import { DossierNumberedSection } from "@/components/issues/IssueDossierPanel";

const DOSSIER_SECTION_LABELS = {
  attention: "Overview",
  nextSteps: "How to fix",
} as const;

export function DossierAttentionSection({ children }: { children: ReactNode }) {
  return (
    <DossierNumberedSection label={DOSSIER_SECTION_LABELS.attention} tone="attention">
      {children}
    </DossierNumberedSection>
  );
}

export function DossierNextStepsSection({ children }: { children: ReactNode }) {
  return (
    <DossierNumberedSection label={DOSSIER_SECTION_LABELS.nextSteps} tone="action">
      {children}
    </DossierNumberedSection>
  );
}

export function DossierVerifyCallout({ children }: { children: ReactNode }) {
  return (
    <div className="nested-info-card">
      <p className="details-section-label">How to check it</p>
      <p className="text-body-muted text-relaxed">{children}</p>
    </div>
  );
}
