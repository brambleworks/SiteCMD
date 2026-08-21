import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Loader2 } from "lucide-react";
import {
  loadCodeBaseline,
  loadCodeFixGuide,
  loadWebBaseline,
  loadWebFixGuide,
} from "@/lib/async-fix-guides";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import type { CodeFixGuide } from "@/lib/code-fix-guides";
import type { FixGuide } from "@/lib/fix-guides";
import { FixGuideSteps } from "./FixGuideSteps";

type Guide = FixGuide | CodeFixGuide;

type AsyncFixGuideStepsProps =
  | {
      kind: "web";
      checkId: string;
      detectedStack?: Record<string, unknown> | null;
      /** Skip catalog resolution and use bundled guidance only. */
      baselineOnly?: boolean;
      fallback?: ReactNode;
      loadingFallback?: ReactNode;
    }
  | {
      kind: "code";
      checkId: string;
      framework?: string | null;
      baselineOnly?: boolean;
      fallback?: ReactNode;
      loadingFallback?: ReactNode;
    };

export function AsyncFixGuideSteps(props: AsyncFixGuideStepsProps) {
  const [guide, setGuide] = useState<Guide | null>(null);
  const [loaded, setLoaded] = useState(false);

  // Flatten union props for stable hook dependencies.
  const detectedStack = props.kind === "web" ? (props.detectedStack ?? null) : null;
  const framework = props.kind === "code" ? (props.framework ?? null) : null;
  const baselineOnly = props.baselineOnly ?? false;

  const stackKey = useMemo(() => {
    if (props.kind === "web") {
      return JSON.stringify(detectedStack);
    }
    return framework ?? "";
  }, [props.kind, detectedStack, framework]);

  // Reload visible catalog-backed guides when a new pack activates.
  const [catalogGeneration, setCatalogGeneration] = useState(0);
  useTauriEvent("catalog-updated", () => setCatalogGeneration((generation) => generation + 1), {
    enabled: !baselineOnly,
  });
  const reloadGeneration = baselineOnly ? 0 : catalogGeneration;

  // Identity changes clear stale guides; generation changes swap content in place.
  const identity = JSON.stringify([props.kind, props.checkId, stackKey, baselineOnly]);
  const stateIdentityRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (stateIdentityRef.current !== identity) {
      stateIdentityRef.current = identity;
      setGuide(null);
      setLoaded(false);
    }

    void (async () => {
      let nextGuide: Guide | null;
      if (baselineOnly) {
        nextGuide =
          props.kind === "web"
            ? await loadWebBaseline(props.checkId)
            : await loadCodeBaseline(props.checkId);
      } else {
        nextGuide =
          props.kind === "web"
            ? await loadWebFixGuide(props.checkId, detectedStack)
            : await loadCodeFixGuide(props.checkId, framework);
      }
      if (cancelled) return;
      setGuide(nextGuide);
      setLoaded(true);
    })();

    return () => {
      cancelled = true;
    };
  }, [
    props.kind,
    props.checkId,
    stackKey,
    detectedStack,
    framework,
    baselineOnly,
    identity,
    reloadGeneration,
  ]);

  if (guide) return <FixGuideSteps guide={guide} />;

  if (!loaded) {
    return (
      <>
        {props.loadingFallback ?? (
          <div className="row text-meta">
            <Loader2 className="icon-xs animate-spin" />
            <span>Loading fix guide…</span>
          </div>
        )}
      </>
    );
  }

  return <>{props.fallback ?? null}</>;
}
