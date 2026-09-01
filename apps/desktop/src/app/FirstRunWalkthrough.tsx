import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Bell,
  Bot,
  Check,
  LayoutDashboard,
  ListChecks,
  PackageCheck,
  Plug,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import type { NavPage, NavTarget } from "@/components/layout/NavSidebar";
import { cn } from "@/lib/utils";

type WalkthroughPage = Extract<
  NavPage,
  "issues" | "updates" | "alerts" | "integrations" | "dashboard"
>;

interface WalkthroughStep {
  id: string;
  page: WalkthroughPage;
  label: string;
  title: string;
  body: string;
  cue: string;
  Icon: typeof LayoutDashboard;
  /** A direct jump for steps whose subject sits behind a tab or deep link. */
  action?: { label: string; target: NavTarget };
}

interface FirstRunWalkthroughProps {
  currentPage: NavPage;
  projectName: string;
  onClose: () => void;
  onNavigate: (target: NavTarget) => void;
}

export function FirstRunWalkthrough({
  currentPage,
  projectName,
  onClose,
  onNavigate,
}: FirstRunWalkthroughProps) {
  const steps = useMemo(() => buildWalkthroughSteps(), []);
  const [stepIndex, setStepIndex] = useState(0);
  const currentStep = steps[stepIndex] ?? steps[0]!;
  const isFirstStep = stepIndex === 0;
  const isLastStep = stepIndex === steps.length - 1;
  const action = currentStep.action;

  // Move to the tour's first page once so its panel always matches the screen.
  const openedRef = useRef(false);
  useEffect(() => {
    if (openedRef.current) return;
    openedRef.current = true;
    const firstStep = steps[0];
    if (firstStep && firstStep.page !== currentPage) {
      onNavigate(firstStep.page);
    }
  }, [currentPage, onNavigate, steps]);

  const goToStep = (nextIndex: number) => {
    const bounded = Math.min(Math.max(0, nextIndex), steps.length - 1);
    const nextStep = steps[bounded] ?? steps[0]!;
    setStepIndex(bounded);
    if (nextStep.page !== currentPage) {
      onNavigate(nextStep.page);
    }
  };

  return (
    <aside className="panel panel--flush walkthrough-panel" aria-label="First run walkthrough">
      <div className="row-between walkthrough-header">
        <div className="min-w-0">
          <p className="section-label-mid text-brand-accent">Quick tour</p>
          <h2 className="text-truncate walkthrough-project">{projectName}</h2>
        </div>
        <Button
          variant="ghost"
          size="icon"
          type="button"
          onClick={onClose}
          className="walkthrough-close text-muted-foreground"
          aria-label="Close walkthrough">
          <X className="icon-md" aria-hidden="true" />
        </Button>
      </div>

      <div className="walkthrough-body">
        <div className="row-between walkthrough-step-head">
          <div className="icon-badge icon-badge--lg walkthrough-step-icon">
            <currentStep.Icon className="icon-lg" aria-hidden="true" />
          </div>
          <div className="walkthrough-step-text">
            <p className="eyebrow walkthrough-step-label">
              Step {stepIndex + 1} of {steps.length}
            </p>
            <h3 className="walkthrough-step-title">{currentStep.title}</h3>
          </div>
        </div>

        <p className="text-body text-relaxed walkthrough-step-body">{currentStep.body}</p>
        <div className="walkthrough-cue ghost-border">
          <p className="walkthrough-cue-text">{currentStep.cue}</p>
        </div>
        {action ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="walkthrough-action"
            onClick={() => onNavigate(action.target)}>
            {action.label}
            <ArrowRight className="icon-sm" aria-hidden="true" />
          </Button>
        ) : null}

        <div className="walkthrough-progress">
          {steps.map((step, index) => (
            <Button
              unstyled
              key={step.id}
              type="button"
              onClick={() => goToStep(index)}
              className={cn(
                "walkthrough-progress-dot",
                index === stepIndex
                  ? "walkthrough-progress-dot--current"
                  : index < stepIndex
                    ? "walkthrough-progress-dot--done"
                    : "walkthrough-progress-dot--todo",
              )}
              aria-label={`Open walkthrough step ${index + 1}: ${step.label}`}
            />
          ))}
        </div>

        <div className="walkthrough-nav">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => goToStep(stepIndex - 1)}
            disabled={isFirstStep}>
            <ArrowLeft className="icon-sm" aria-hidden="true" />
            Back
          </Button>

          {isLastStep ? (
            <Button type="button" size="sm" onClick={onClose}>
              <Check className="icon-sm" aria-hidden="true" />
              Finish
            </Button>
          ) : (
            <Button type="button" size="sm" onClick={() => goToStep(stepIndex + 1)}>
              Next: {steps[stepIndex + 1]?.label}
              <ArrowRight className="icon-sm" aria-hidden="true" />
            </Button>
          )}
        </div>
      </div>
    </aside>
  );
}

// The first scan lands on Issues; future sessions begin on Dashboard.
function buildWalkthroughSteps(): WalkthroughStep[] {
  return [
    {
      id: "issues",
      page: "issues",
      label: "Issues",
      title: "Start with what the scan found",
      body: "This is the to-do list from your first scan. Web problems and code problems are mixed together so the most important stuff rises to the top.",
      cue: "Click an issue row to see what is wrong, why it matters, and what to do next.",
      Icon: ListChecks,
    },
    {
      id: "updates",
      page: "updates",
      label: "Updates",
      title: "Check your packages",
      body: "This shows packages that are out of date, especially the security fixes you should not miss.",
      cue: "Start with Security and Major updates. Patch and Minor updates can usually wait unless something broke.",
      Icon: PackageCheck,
    },
    {
      id: "alerts",
      page: "alerts",
      label: "Alerts",
      title: "See what changed while you were away",
      body: "Alerts collects regressions, failed scans, security updates, and signals from connected services in one place, so change finds you instead of the other way around.",
      cue: "Empty right now is a good thing. Check back after scheduled scans run or when the sidebar badge lights up.",
      Icon: Bell,
    },
    {
      id: "ai-editor",
      page: "integrations",
      label: "AI editor",
      title: "Connect your AI editor",
      body: "Claude Code, Cursor, Codex, and Windsurf can pull an issue, fix it in your code, and report back so SiteCMD can verify the fix. The connection is one small config entry that SiteCMD writes for you, or that you paste yourself.",
      cue: "Under Agent tools, connect the editor you already use. No editor detected? Open Manual setup and copy the config block.",
      Icon: Bot,
      action: { label: "Open agent tools", target: "settings:integrations" },
    },
    {
      id: "integrations",
      page: "integrations",
      label: "Integrations",
      title: "Connect your services",
      body: "Connect your AI agent so it can pull and fix Issues in your code, then send them back for verification. Analytics, search, uptime, and GitHub connections add real-world context and feed Alerts.",
      cue: "You do not need everything. Start with the one service you already check most, like analytics or Search Console.",
      Icon: Plug,
    },
    {
      id: "dashboard",
      page: "dashboard",
      label: "Dashboard",
      title: "Your home base",
      body: "This is your home base. Use it when you want to know what needs attention without checking five different places.",
      cue: "Look at the Issues and Updates cards first. Those are the main places you will work.",
      Icon: LayoutDashboard,
    },
  ];
}
