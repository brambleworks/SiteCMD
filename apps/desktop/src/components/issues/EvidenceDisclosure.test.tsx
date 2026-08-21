import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EvidenceDisclosure } from "./EvidenceDisclosure";
import type { Evidence } from "@/lib/types";

describe("EvidenceDisclosure", () => {
  it("renders nothing when evidence is empty", () => {
    const { container } = render(<EvidenceDisclosure evidence={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders the count in the summary", () => {
    const evidence: Evidence[] = [
      { kind: "scan", timestamp: null, source: "sitecmd", detail: "LCP measured at 4.2s" },
      { kind: "observation", timestamp: null, source: "sitecmd", detail: "Seen in 3 scans" },
    ];
    render(<EvidenceDisclosure evidence={evidence} />);
    expect(screen.getByText(/Show evidence \(2\)/)).toBeInTheDocument();
  });

  it("renders each evidence row with kind and detail", () => {
    const evidence: Evidence[] = [
      { kind: "scan", timestamp: null, source: "sitecmd", detail: "LCP measured at 4.2s" },
      { kind: "observation", timestamp: null, source: "sitecmd", detail: "Seen in 3 scans" },
    ];
    render(<EvidenceDisclosure evidence={evidence} />);
    expect(screen.getByText("scan")).toBeInTheDocument();
    expect(screen.getByText("LCP measured at 4.2s")).toBeInTheDocument();
    expect(screen.getByText("observation")).toBeInTheDocument();
    expect(screen.getByText("Seen in 3 scans")).toBeInTheDocument();
  });

  it("renders timestamp when present", () => {
    const iso = new Date(Date.now() - 30 * 60_000).toISOString();
    const evidence: Evidence[] = [
      { kind: "scan", timestamp: iso, source: "sitecmd", detail: "LCP measured at 4.2s" },
    ];
    render(<EvidenceDisclosure evidence={evidence} />);
    expect(screen.getByText(/m ago|h ago|d ago/)).toBeInTheDocument();
  });

  it("omits timestamp when null", () => {
    const evidence: Evidence[] = [
      { kind: "scan", timestamp: null, source: "sitecmd", detail: "LCP measured at 4.2s" },
    ];
    const { container } = render(<EvidenceDisclosure evidence={evidence} />);
    expect(container.querySelector(".ev-timestamp")).toBeNull();
  });
});
