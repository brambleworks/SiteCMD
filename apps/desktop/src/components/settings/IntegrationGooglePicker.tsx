import { useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import {
  filterGooglePickerData,
  googleIntegrationLabel,
  pickPreferredGoogleChoice,
  sortSearchConsoleSites,
  type GoogleIntegrationType,
  type GooglePickerData,
} from "./google-integration-selection";

interface GooglePickerProps {
  data: GooglePickerData;
  connectedTypes: Set<string>;
  projectHost: string;
  targetType?: GoogleIntegrationType | null;
  onPick: (type: string, id: string) => void;
  onClose: () => void;
}

export function GooglePicker({
  data,
  connectedTypes,
  projectHost,
  targetType = null,
  onPick,
  onClose,
}: GooglePickerProps) {
  const visibleData = filterGooglePickerData(data, targetType);
  const sortedGsc = sortSearchConsoleSites(visibleData.gsc_sites, projectHost);

  const showGa4 = visibleData.ga4_properties.length > 0 && !connectedTypes.has("googleanalytics");
  const showGsc = sortedGsc.length > 0 && !connectedTypes.has("googlesearchconsole");
  const hasAnyItems = showGa4 || showGsc;

  const [selectedGa4, setSelectedGa4] = useState("");
  const [selectedGsc, setSelectedGsc] = useState<string>(
    () => pickPreferredGoogleChoice(visibleData, "googlesearchconsole", projectHost) ?? "",
  );

  const title = targetType
    ? `Choose a ${googleIntegrationLabel(targetType)} ${targetType === "googleanalytics" ? "property" : "site"}`
    : "Choose what to connect";

  const canConnect =
    hasAnyItems && ((showGa4 && selectedGa4 !== "") || (showGsc && selectedGsc !== ""));

  const handleConnect = () => {
    if (showGa4 && selectedGa4) {
      onPick("googleanalytics", selectedGa4);
    }
    if (showGsc && selectedGsc) {
      onPick("googlesearchconsole", selectedGsc);
    }
  };

  const emptyMessage = () => {
    if (targetType) {
      const err = targetType === "googleanalytics" ? visibleData.ga4_error : visibleData.gsc_error;
      if (err) return `Could not load your Google data: ${err}`;
      return `No ${googleIntegrationLabel(targetType)} ${targetType === "googleanalytics" ? "properties" : "sites"} found for this Google account.`;
    }
    return "No properties or sites found for this Google account.";
  };

  return (
    <Dialog
      labelledBy="google-picker-title"
      onClose={onClose}
      backdropClassName="dialog--soft"
      className="fix-prompt-modal">
      <div className="fix-prompt-modal-header">
        <h3 id="google-picker-title" className="fix-prompt-modal-title">
          {title}
        </h3>
        <Button
          unstyled
          type="button"
          className="details-close"
          aria-label="Close"
          onClick={onClose}>
          <X />
        </Button>
      </div>

      <div className="agent-handoff-body">
        {hasAnyItems ? (
          <>
            {showGa4 && (
              <div className="google-picker-field">
                <label htmlFor="ga4-property-select" className="section-label-mid">
                  Google Analytics property
                </label>
                <select
                  id="ga4-property-select"
                  className="field-control field-control--select"
                  value={selectedGa4}
                  onChange={(e) => setSelectedGa4(e.target.value)}>
                  <option value="">{targetType ? "Choose a property" : "Do not connect"}</option>
                  {visibleData.ga4_properties.map((prop) => (
                    <option key={prop.property_id} value={prop.property_id}>
                      {prop.display_name} ({prop.account_name})
                    </option>
                  ))}
                </select>
              </div>
            )}

            {showGsc && (
              <div className="google-picker-field">
                <label htmlFor="gsc-site-select" className="section-label-mid">
                  Search Console site
                </label>
                <select
                  id="gsc-site-select"
                  className="field-control field-control--select"
                  value={selectedGsc}
                  onChange={(e) => setSelectedGsc(e.target.value)}>
                  <option value="">{targetType ? "Choose a site" : "Do not connect"}</option>
                  {sortedGsc.map((site) => (
                    <option key={site.site_url} value={site.site_url}>
                      {site.site_url}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </>
        ) : (
          <p className="body-muted">{emptyMessage()}</p>
        )}
      </div>

      <div className="fix-prompt-modal-footer">
        <Button variant="outline" type="button" onClick={onClose}>
          Cancel
        </Button>
        {hasAnyItems && (
          <Button type="button" onClick={handleConnect} disabled={!canConnect}>
            Connect
          </Button>
        )}
      </div>
    </Dialog>
  );
}
