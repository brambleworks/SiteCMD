import type { IssuesTab } from "@/pages/issues/issues-page-model";
import { Button } from "@/components/ui/button";

interface IssuesTabBarProps {
  activeTab: IssuesTab;
  onSwitch: (tab: IssuesTab) => void;
}

export function IssuesTabBar({ activeTab, onSwitch }: IssuesTabBarProps) {
  return (
    <div className="tab-bar">
      <Button
        unstyled
        type="button"
        onClick={() => onSwitch("issues")}
        className={`tab ${activeTab === "issues" ? "tab-active" : "tab-inactive"}`}>
        Issues
      </Button>
      <Button
        unstyled
        type="button"
        onClick={() => onSwitch("by-page")}
        className={`tab ${activeTab === "by-page" ? "tab-active" : "tab-inactive"}`}>
        Pages
      </Button>
      <Button
        unstyled
        type="button"
        onClick={() => onSwitch("history")}
        className={`tab ${activeTab === "history" ? "tab-active" : "tab-inactive"}`}>
        History
      </Button>
    </div>
  );
}
