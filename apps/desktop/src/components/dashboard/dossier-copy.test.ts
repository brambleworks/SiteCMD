import { describe, expect, it } from "vitest";
import { isDuplicateDossierCopy, pickSupportingDossierCopy } from "./dossier-copy";

describe("dossier copy helpers", () => {
  it("treats repeated description copy as duplicate even with punctuation changes", () => {
    expect(
      isDuplicateDossierCopy(
        "CSP prevents cross-site scripting (XSS) by controlling which scripts and resources can load.",
        "CSP prevents cross-site scripting XSS by controlling which scripts and resources can load",
      ),
    ).toBe(true);
  });

  it("picks the first useful supporting line that does not repeat the description", () => {
    const description =
      "CSP prevents cross-site scripting (XSS) by controlling which scripts and resources can load.";

    expect(
      pickSupportingDossierCopy(description, [
        description,
        "Without a CSP header, injected scripts can run in the user's browser.",
      ]),
    ).toBe("Without a CSP header, injected scripts can run in the user's browser.");
  });

  it("returns null when all supporting copy repeats the description", () => {
    const description = "Every page should have a unique title tag.";

    expect(
      pickSupportingDossierCopy(description, [
        description,
        "Every page should have a unique title tag",
      ]),
    ).toBeNull();
  });
});
