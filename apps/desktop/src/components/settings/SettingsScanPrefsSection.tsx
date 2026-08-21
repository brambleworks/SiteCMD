import { ScanScheduleCard } from "@/components/scan/ScanScheduleCard";
import { useScanPrefs } from "@/hooks/useScanPrefs";

const RETENTION_MAX = 100;

interface ScanningSectionProps {
  projectId?: number;
  environmentId?: number;
  projectPath?: string | null;
}

export function ScanningSection({ projectId, environmentId, projectPath }: ScanningSectionProps) {
  const { prefs, setPrefs } = useScanPrefs();
  const retentionMax = RETENTION_MAX;
  const effectiveRetention = Math.min(prefs.retentionLimit, retentionMax);

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious">
        <SettingsPanelHeader
          title="Scan Behavior"
          description="Set the defaults that apply when you run a Web Scan from the app."
        />
        <div className="settings-range-control">
          <div className="flex-fill">
            <p className="row-title-md">Per-check timeout</p>
            <p className="subtitle-xs">
              Use a longer timeout for slow staging sites, protected environments, or hosts that
              throttle checks.
            </p>
          </div>
          <div className="settings-range-control-input">
            <input
              type="range"
              min={10}
              max={60}
              step={5}
              value={prefs.timeout}
              onChange={(e) => setPrefs({ ...prefs, timeout: Number(e.target.value) })}
              className="settings-range-slider"
            />
            <span className="row-title-lg settings-range-value">{prefs.timeout}s</span>
          </div>
        </div>
        <div className="settings-range-control subtle-divider-top settings-range-control--divided">
          <div className="flex-fill">
            <p className="row-title-md">Scan history to keep</p>
            <p className="subtitle-xs">
              {`Keep the newest ${effectiveRetention} scans per site environment. Older runs are removed automatically.`}
            </p>
          </div>
          <div className="settings-range-control-input">
            <input
              type="range"
              min={5}
              max={retentionMax}
              step={5}
              value={effectiveRetention}
              onChange={(e) => setPrefs({ ...prefs, retentionLimit: Number(e.target.value) })}
              className="settings-range-slider"
            />
            <span className="row-title-lg settings-range-value">{effectiveRetention}</span>
          </div>
        </div>
      </section>

      <ScanScheduleCard
        projectId={projectId}
        environmentId={environmentId}
        projectPath={projectPath}
      />
    </div>
  );
}

function SettingsPanelHeader({ title, description }: { title: string; description: string }) {
  return (
    <div className="settings-panel-header">
      <div className="flex-fill">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">{title}</h2>
        </div>
        <p className="body-muted settings-card-desc">{description}</p>
      </div>
    </div>
  );
}
