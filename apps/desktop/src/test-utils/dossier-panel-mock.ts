import React from "react";

interface DossierPanelMockOptions {
  testId?: string;
}

export function buildDossierPanelMock({ testId = "issue-dossier" }: DossierPanelMockOptions = {}) {
  return {
    IssueDossierPanel: ({
      title,
      children,
      footer,
      leftRail,
      rightRail,
    }: {
      title: string;
      children?: React.ReactNode;
      footer?: React.ReactNode;
      leftRail?: React.ReactNode;
      rightRail?: React.ReactNode;
    }) =>
      React.createElement("div", { "data-testid": testId }, [
        React.createElement("div", { key: "title" }, title),
        React.createElement("div", { key: "left" }, leftRail),
        React.createElement("div", { key: "children" }, children),
        React.createElement("div", { key: "right" }, rightRail),
        React.createElement("div", { key: "footer" }, footer),
      ]),
    DossierSection: ({
      label,
      children,
      action,
    }: {
      label: string;
      children?: React.ReactNode;
      action?: React.ReactNode;
    }) =>
      React.createElement("section", null, [
        React.createElement("h3", { key: "label" }, label),
        action ? React.createElement("div", { key: "action" }, action) : null,
        React.createElement("div", { key: "children" }, children),
      ]),
    DossierNumberedSection: ({
      index,
      label,
      children,
    }: {
      index: number;
      label: string;
      children?: React.ReactNode;
    }) =>
      React.createElement("section", null, [
        React.createElement("h3", { key: "label" }, `${String(index).padStart(2, "0")} - ${label}`),
        React.createElement("div", { key: "children" }, children),
      ]),
    DossierRail: ({ label, children }: { label: string; children?: React.ReactNode }) =>
      React.createElement("section", null, [
        React.createElement("h4", { key: "label" }, label),
        React.createElement("div", { key: "children" }, children),
      ]),
    DossierKeyValueGrid: () => React.createElement("div", null, "DossierKeyValueGrid"),
  };
}
