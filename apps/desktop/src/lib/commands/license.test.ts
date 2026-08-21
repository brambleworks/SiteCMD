import { describe, expect, it, vi } from "vitest";

const { command } = vi.hoisted(() => ({ command: vi.fn() }));

vi.mock("./invoke", () => ({ command }));

import { validateLicense } from "./license";

describe("validateLicense", () => {
  it("passes force through to the backend when the gesture asks for a live check", async () => {
    command.mockClear();
    await validateLicense({ force: true });
    expect(command).toHaveBeenCalledWith("validate_license", { force: true });
  });

  it("stays a bare no-arg invoke when nothing forces", async () => {
    command.mockClear();
    await validateLicense();
    expect(command).toHaveBeenCalledTimes(1);
    expect(command.mock.calls[0]).toEqual(["validate_license"]);
  });
});
