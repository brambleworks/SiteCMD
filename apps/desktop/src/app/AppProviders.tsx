import React, { useCallback, useEffect } from "react";

import { useHistory } from "@/hooks/useHistory";
import { ProjectProvider } from "@/hooks/useProject";
import { useScan } from "@/hooks/useScan";
import { TierProvider } from "@/hooks/useTier";

export interface AppShellHooks {
  scanHook: ReturnType<typeof useScan>;
  historyHook: ReturnType<typeof useHistory>;
}

interface AppProvidersProps {
  children: (hooks: AppShellHooks) => React.ReactNode;
}

export function AppProviders({ children }: AppProvidersProps) {
  const scanHook = useScan();
  const historyHook = useHistory();

  const scanRef = React.useRef(scanHook);

  useEffect(() => {
    scanRef.current = scanHook;
  }, [scanHook]);

  const handleEnvChange = useCallback(
    (_env: import("@/hooks/useProject").EnvironmentRecord | null) => {
      if (scanRef.current.state === "scanning") {
        void scanRef.current.cancelScan();
      } else {
        scanRef.current.reset();
      }
    },
    [],
  );

  return (
    <TierProvider>
      <ProjectProvider onEnvChange={handleEnvChange}>
        {children({ scanHook, historyHook })}
      </ProjectProvider>
    </TierProvider>
  );
}
