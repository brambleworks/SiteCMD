import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ServiceIconWithBg } from "./ServiceIcon";

describe("ServiceIconWithBg", () => {
  it.each([
    ["plausible", "service-icon--plausible"],
    ["cloudflare", "service-icon--cloudflare"],
    ["uptimerobot", "service-icon--uptimerobot"],
    ["bingwebmaster", "service-icon--bing"],
    ["github", "service-icon--github"],
  ])("uses the semantic color class for %s", (type, className) => {
    const { container } = render(<ServiceIconWithBg type={type} />);
    expect(container.firstElementChild).toHaveClass(className);
  });

  it("uses the muted color for an unknown service", () => {
    const { container } = render(<ServiceIconWithBg type="unknown" />);
    expect(container.firstElementChild).toHaveClass("text-muted-foreground");
  });
});
