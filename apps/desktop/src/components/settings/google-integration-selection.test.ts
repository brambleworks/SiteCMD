import { describe, expect, it } from "vitest";
import {
  pickPreferredGoogleChoice,
  sortSearchConsoleSites,
  type GooglePickerData,
} from "./google-integration-selection";

function dataWithSites(...sites: string[]): GooglePickerData {
  return {
    ga4_properties: [
      { property_id: "properties/111", display_name: "Other site", account_name: "Account" },
    ],
    gsc_sites: sites.map((site_url) => ({ site_url, permission: "siteOwner" })),
  };
}

describe("Google integration selection", () => {
  it("never assumes a lone Analytics property belongs to the project", () => {
    expect(pickPreferredGoogleChoice(dataWithSites(), "googleanalytics", "example.com")).toBeNull();
  });

  it.each(["other.com", ""])("does not suggest a lone Search Console site for %s", (host) => {
    expect(
      pickPreferredGoogleChoice(dataWithSites("https://example.com/"), "googlesearchconsole", host),
    ).toBeNull();
  });

  it("prefers one matching Search Console site among unrelated sites", () => {
    const data = dataWithSites("https://other.com/", "https://example.com/");
    expect(pickPreferredGoogleChoice(data, "googlesearchconsole", "example.com")).toBe(
      "https://example.com/",
    );
    expect(sortSearchConsoleSites(data.gsc_sites, "example.com")[0]?.site_url).toBe(
      "https://example.com/",
    );
  });

  it("matches a domain property to a project on a subdomain", () => {
    expect(
      pickPreferredGoogleChoice(
        dataWithSites("sc-domain:example.com"),
        "googlesearchconsole",
        "app.example.com",
      ),
    ).toBe("sc-domain:example.com");
  });

  it("requires a choice when multiple Search Console properties match", () => {
    expect(
      pickPreferredGoogleChoice(
        dataWithSites("https://example.com/", "sc-domain:example.com"),
        "googlesearchconsole",
        "example.com",
      ),
    ).toBeNull();
  });

  it.each(["https://child.example.com/", "not a site example.com", "sc-domain:otherexample.com"])(
    "does not treat %s as the project site",
    (site) => {
      expect(
        pickPreferredGoogleChoice(dataWithSites(site), "googlesearchconsole", "example.com"),
      ).toBeNull();
    },
  );
});
