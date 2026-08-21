import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AlertDetail } from "./AlertDetail";
import type { AlertItem } from "@/lib/issue-ranking";
import type { PackageUpdate } from "@/lib/types";

const vulnAlert: AlertItem = {
  id: "vuln-packages",
  priority: "critical",
  title: "Critical package security update",
  description: "lodash needs a security update.",
  action: "Open Updates",
};

const packageUpdate: PackageUpdate = {
  name: "lodash",
  currentVersion: "4.17.19",
  latestVersion: "4.17.21",
  ecosystem: "npm",
  updateType: "patch",
  isSecurity: true,
  advisorySeverity: "critical",
  advisoryFixedVersion: "4.17.21",
  advisoryUrl: null,
  source: "package-lock.json",
  isDev: false,
  isDeprecated: false,
  deprecationMessage: null,
  currentVersionDeprecated: false,
  isStale: false,
  lastPublished: null,
  workspaceMembers: [],
};

describe("AlertDetail", () => {
  it("labels an OSV-verified package version as the fixed release", () => {
    render(<AlertDetail alert={vulnAlert} securityUpdates={[packageUpdate]} />);

    expect(screen.getByText(/Fixed release:/)).toBeInTheDocument();
    expect(screen.queryByText("Safe:")).not.toBeInTheDocument();
    expect(screen.getByText("4.17.21")).toBeInTheDocument();
  });

  it("shows mitigation guidance instead of an upgrade command without a fixed release", () => {
    const { advisoryFixedVersion: _, ...withoutFix } = packageUpdate;
    render(<AlertDetail alert={vulnAlert} securityUpdates={[withoutFix]} />);

    expect(screen.getByText("not published")).toBeInTheDocument();
    expect(screen.getByText(/review the advisory for mitigations/i)).toBeInTheDocument();
    expect(screen.queryByText(/npm install/i)).not.toBeInTheDocument();
  });

  it("uses renewal language for SSL alerts that may already be expired", () => {
    render(
      <AlertDetail
        alert={{
          id: "ssl-expiry",
          priority: "critical",
          title: "SSL certificate expired",
          description: "Certificate expired.",
          action: "Open Security",
        }}
      />,
    );

    expect(screen.getByText("SSL certificate needs renewal")).toBeInTheDocument();
    expect(screen.getByText(/expired or inside the renewal window/i)).toBeInTheDocument();
  });
});
