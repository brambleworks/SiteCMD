import { render, screen } from "@testing-library/react";
import React from "react";
import { describe, expect, it, vi } from "vitest";
import type { ConnectedStatus } from "@/generated/ipc-bindings-connected";

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: vi.fn(), error: vi.fn() }),
}));
vi.mock("@/components/ui/external-link", () => ({
  ExtLink: ({ children, href }: { children: React.ReactNode; href: string }) =>
    React.createElement("a", { href }, children),
}));

import { ConnectedServiceSetup } from "./ConnectedServiceSetup";

const localOnly: ConnectedStatus = {
  endpointConfigured: true,
  connected: false,
  siteId: null,
  bootstrapped: false,
  hasInstallationToken: false,
  hasFingerprintKey: false,
  pendingMutations: 0,
  conflictedMutations: 0,
  pendingScopeSync: false,
  lastSubmissionSequence: 0,
  fingerprintKeyVersion: 1,
  pendingKeyVersion: null,
};

function renderSetup() {
  return render(
    <ConnectedServiceSetup
      scope={{ projectId: 7, environmentScopeKey: "https://example.com" }}
      status={localOnly}
      onChallenge={() => undefined}
      onStatusChanged={() => Promise.resolve()}
    />,
  );
}

describe("ConnectedServiceSetup", () => {
  it("shows the terms and privacy policy on every path that connects a site", () => {
    renderSetup();

    const terms = screen.getAllByRole("link", { name: "Terms of Service" });
    const privacy = screen.getAllByRole("link", { name: "Privacy Policy" });
    expect(terms).toHaveLength(2);
    expect(privacy).toHaveLength(2);
    for (const link of terms) expect(link).toHaveAttribute("href", "https://sitecmd.com/terms");
    for (const link of privacy) {
      expect(link).toHaveAttribute("href", "https://sitecmd.com/privacy");
    }
  });
});
