import { useCallback, useEffect, useState } from "react";

const REFERENCE_SIGNALS_DELAY_MS = 3000;
const AUXILIARY_SIGNALS_DELAY_MS = 9000;

export function useDashboardSignalArming({
  dashboardReady,
  includeReferenceSignals,
  projectId,
  url,
}: {
  dashboardReady: boolean;
  includeReferenceSignals: boolean;
  projectId: number;
  url: string;
}) {
  const [referenceSignalsArmed, setReferenceSignalsArmed] = useState(false);
  const [auxiliarySignalsArmed, setAuxiliarySignalsArmed] = useState(false);

  const disarmSignals = useCallback(() => {
    setReferenceSignalsArmed(false);
    setAuxiliarySignalsArmed(false);
  }, []);

  useEffect(() => {
    if (!includeReferenceSignals || !dashboardReady) return;

    const timeoutId = window.setTimeout(() => {
      setReferenceSignalsArmed(true);
    }, REFERENCE_SIGNALS_DELAY_MS);

    return () => window.clearTimeout(timeoutId);
  }, [dashboardReady, includeReferenceSignals, projectId, url]);

  useEffect(() => {
    if (!includeReferenceSignals || !dashboardReady) return;

    const timeoutId = window.setTimeout(() => {
      setAuxiliarySignalsArmed(true);
    }, AUXILIARY_SIGNALS_DELAY_MS);

    return () => window.clearTimeout(timeoutId);
  }, [dashboardReady, includeReferenceSignals, projectId, url]);

  return { auxiliarySignalsArmed, disarmSignals, referenceSignalsArmed };
}
