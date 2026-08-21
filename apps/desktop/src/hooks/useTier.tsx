import {
  createContext,
  useContext,
  useEffect,
  useState,
  useMemo,
  useCallback,
  type ReactNode,
} from "react";
import {
  activateLicense as activateLicenseCommand,
  deactivateLicense as deactivateLicenseCommand,
  getLicenseStatus,
  validateLicense,
} from "@/lib/commands";
import { MS_PER_HOUR } from "@/lib/format";
import { onLatePrivilegedResolution } from "@/lib/privileged-command-bridge";
import { setTelemetryTier } from "@/lib/telemetry";

export type Tier = "free" | "core" | "pro";
export type BillingInterval = "monthly" | "yearly";

interface CheckoutUrls {
  core: string;
  pro: string;
  coreMonthly: string;
  coreAnnual: string;
  proMonthly: string;
  proAnnual: string;
}

// Re-export the generated wire type used by the stale-validation banner.
import type { ValidationWarning } from "@/generated/ipc-bindings";
export type { ValidationWarning };

interface LicenseInfo {
  tier: Tier;
  status: string;
  planName: string;
  billingInterval: BillingInterval | null;
  isActive: boolean;
  expiresAt: string | null;
  checkoutUrls: CheckoutUrls;
  customerPortalUrl: string;
  validationWarning: ValidationWarning;
}

const FREE_LICENSE: LicenseInfo = {
  tier: "free",
  status: "none",
  planName: "Free",
  billingInterval: null,
  isActive: false,
  expiresAt: null,
  checkoutUrls: {
    core: "",
    pro: "",
    coreMonthly: "",
    coreAnnual: "",
    proMonthly: "",
    proAnnual: "",
  },
  customerPortalUrl: "",
  validationWarning: "none",
};

interface TierContextValue {
  /** Current license tier. Lemon Squeezy owns any checkout-level trial period. */
  tier: Tier;
  licenseInfo: LicenseInfo;
  isLoading: boolean;
  activateLicense: (key: string) => Promise<LicenseInfo>;
  deactivateLicense: () => Promise<void>;
  /** Revalidate, optionally bypassing the cache for an explicit user refresh. */
  refreshLicense: (options?: { force?: boolean }) => Promise<LicenseInfo | null>;
}

const TierContext = createContext<TierContextValue | null>(null);

export function TierProvider({ children }: { children: ReactNode }) {
  const [licenseInfo, setLicenseInfo] = useState<LicenseInfo>(FREE_LICENSE);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    // Revalidate stale cache-only startup state in the background.
    async function backgroundRevalidate() {
      try {
        const fresh = await validateLicense();
        if (!cancelled) setLicenseInfo(fresh);
      } catch {
        // Keep cached state and its warning.
      }
    }

    async function loadAll() {
      try {
        const cached = await getLicenseStatus();
        if (!cancelled) {
          setLicenseInfo(cached);
        }
        if (
          cached.validationWarning === "stale" ||
          cached.validationWarning === "stale_final_warning"
        ) {
          void backgroundRevalidate();
        }
      } catch {
        // Keep the Free fallback.
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    }

    loadAll();

    // The backend cache gate keeps this cheap while long sessions stay fresh.
    const SIX_HOURS_MS = 6 * MS_PER_HOUR;
    const interval = window.setInterval(() => {
      void backgroundRevalidate();
    }, SIX_HOURS_MS);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  // Apply native verdicts that arrive after the bridge deadline.
  useEffect(() => {
    return onLatePrivilegedResolution((late) => {
      if (late.command !== "validate_license" || !late.ok) return;
      if (late.value && typeof late.value === "object" && "tier" in late.value) {
        setLicenseInfo(late.value as LicenseInfo);
      }
    });
  }, []);

  const effectiveTier: Tier = licenseInfo.tier;

  // Telemetry cannot read React context directly.
  useEffect(() => {
    setTelemetryTier(effectiveTier);
  }, [effectiveTier]);

  const activateLicense = useCallback(async (key: string) => {
    const info = await activateLicenseCommand({ key });
    setLicenseInfo(info);
    return info;
  }, []);

  const deactivateLicense = useCallback(async () => {
    await deactivateLicenseCommand();
    setLicenseInfo(FREE_LICENSE);
  }, []);

  const refreshLicense = useCallback(
    async (options?: { force?: boolean }): Promise<LicenseInfo | null> => {
      try {
        const info = await validateLicense(options);
        setLicenseInfo(info);
        return info;
      } catch {
        // Keep the current tier.
        return null;
      }
    },
    [],
  );

  const value = useMemo(
    () => ({
      tier: effectiveTier,
      licenseInfo,
      isLoading,
      activateLicense,
      deactivateLicense,
      refreshLicense,
    }),
    [effectiveTier, licenseInfo, isLoading, activateLicense, deactivateLicense, refreshLicense],
  );

  return <TierContext.Provider value={value}>{children}</TierContext.Provider>;
}

export function useTier(): TierContextValue {
  const ctx = useContext(TierContext);
  if (!ctx) {
    throw new Error("useTier() must be used within a <TierProvider>");
  }
  return ctx;
}
