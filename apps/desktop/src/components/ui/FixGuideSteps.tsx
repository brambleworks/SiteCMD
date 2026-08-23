import type { FixGuide } from "@/lib/fix-guides";
import { copyToClipboard } from "@/lib/clipboard";
import { Check, Copy } from "lucide-react";
import { useState, useCallback, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

type StepSegment =
  | { type: "text"; value: string }
  | { type: "code"; value: string; lang?: string }
  | { type: "inline-code"; value: string };

function parseStep(raw: string): StepSegment[] {
  const segments: StepSegment[] = [];
  let cursor = 0;

  while (cursor < raw.length) {
    const blockStart = raw.indexOf("```", cursor);

    if (blockStart === -1) {
      const rest = raw.slice(cursor);
      if (rest) segments.push(...parseInlineCode(rest));
      break;
    }

    if (blockStart > cursor) {
      segments.push(...parseInlineCode(raw.slice(cursor, blockStart)));
    }

    const langEnd = raw.indexOf("\n", blockStart + 3);
    if (langEnd === -1) {
      segments.push(...parseInlineCode(raw.slice(blockStart)));
      break;
    }

    const lang = raw.slice(blockStart + 3, langEnd).trim() || undefined;
    const blockEnd = raw.indexOf("```", langEnd + 1);

    if (blockEnd === -1) {
      segments.push({ type: "code", value: raw.slice(langEnd + 1).trimEnd(), lang });
      break;
    }

    segments.push({ type: "code", value: raw.slice(langEnd + 1, blockEnd).trimEnd(), lang });
    cursor = blockEnd + 3;
  }

  return segments;
}

function parseInlineCode(text: string): StepSegment[] {
  const segments: StepSegment[] = [];
  const parts = text.split(/`([^`]+)`/);
  for (let i = 0; i < parts.length; i++) {
    if (i % 2 === 0) {
      if (parts[i]) segments.push({ type: "text", value: parts[i] });
    } else {
      segments.push({ type: "inline-code", value: parts[i] });
    }
  }
  return segments;
}

function CodeBlock({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    const ok = await copyToClipboard(code);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  }, [code]);

  return (
    <div className="fix-guide-code-block group">
      <pre className="fix-guide-code">{code}</pre>
      <Button unstyled type="button" onClick={handleCopy} className="code-copy-button">
        {copied ? <Check className="icon-xs text-score-excellent" /> : <Copy className="icon-xs" />}
      </Button>
    </div>
  );
}

function StepContent({ step }: { step: string }) {
  const segments = parseStep(step);
  const nodes: ReactNode[] = [];

  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (seg.type === "text") {
      nodes.push(<span key={i}>{seg.value}</span>);
    } else if (seg.type === "inline-code") {
      nodes.push(
        <code key={i} className="inline-code-token">
          {seg.value}
        </code>,
      );
    } else {
      nodes.push(<CodeBlock key={i} code={seg.value} />);
    }
  }

  return <>{nodes}</>;
}

export function FixGuideSteps({ guide }: { guide: FixGuide }) {
  return (
    <div>
      {guide.lead ? <p className="fix-guide-lead body-text">{guide.lead}</p> : null}
      <ol className="fix-guide-steps">
        {guide.steps.map((step, i) => (
          <li key={i} className="body-text">
            <span className="fix-guide-step-num">{i + 1}.</span>
            <div className="flex-fill">
              <StepContent step={step} />
            </div>
          </li>
        ))}
      </ol>
    </div>
  );
}
