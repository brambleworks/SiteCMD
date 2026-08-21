import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  PDF_MUTED,
  PDF_SCORE,
  PDF_SEVERITY,
  pdfScoreColor,
  pdfSeverityColor,
} from "./report-pdf-colors";
import { scoreColor, severityColor } from "./report-pdf-model";

const HEX = /^#[0-9a-f]{6}$/i;

/** WCAG 2.x relative luminance / contrast ratio against a background. */
function channel(c: number): number {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}
function luminance(hex: string): number {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}
function contrastOnWhite(hex: string): number {
  const l = luminance(hex);
  return 1.05 / (l + 0.05);
}

describe("PDF report colors resolve to concrete hex", () => {
  it("pdfScoreColor returns hex across every band", () => {
    for (const score of [95, 75, 55, 35, 10]) {
      expect(pdfScoreColor(score)).toMatch(HEX);
    }
  });

  it("pdfSeverityColor returns hex for known severities and muted for unknown", () => {
    for (const severity of ["critical", "high", "medium", "low"]) {
      expect(pdfSeverityColor(severity)).toMatch(HEX);
    }
    expect(pdfSeverityColor("bogus")).toBe(PDF_MUTED);
    expect(PDF_MUTED).toMatch(HEX);
  });

  it("the legacy scoreColor/severityColor re-exports never emit var()", () => {
    expect(scoreColor(95)).toMatch(HEX);
    expect(scoreColor(95)).not.toContain("var(");
    expect(severityColor("critical")).toMatch(HEX);
    expect(severityColor("critical")).not.toContain("var(");
  });
});

describe("PDF colors are legible on the white report page", () => {
  const allColors = { ...PDF_SCORE, ...PDF_SEVERITY, muted: PDF_MUTED };
  it.each(Object.entries(allColors))("%s (%s) clears 4.5:1 on white", (_name, hex) => {
    expect(contrastOnWhite(hex)).toBeGreaterThanOrEqual(4.5);
  });
});

describe("no CSS custom properties leak into the PDF pipeline", () => {
  // react-pdf renders CSS custom-property colors as black.
  const here = dirname(fileURLToPath(import.meta.url));
  const pdfPipeline = readdirSync(here).filter(
    (name) => /^(report-pdf-|ReportPDF).*\.(ts|tsx)$/.test(name) && !name.includes(".test."),
  );

  it("finds the PDF pipeline files", () => {
    expect(pdfPipeline.length).toBeGreaterThanOrEqual(4);
  });

  it.each(pdfPipeline)("%s contains no var(--...)", (name) => {
    // Inspect code only.
    const source = readFileSync(join(here, name), "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "");
    expect(source).not.toMatch(/var\(--/);
  });
});
