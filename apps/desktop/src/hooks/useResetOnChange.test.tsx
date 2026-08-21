import { render } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { useResetOnChange } from "./useResetOnChange";

describe("useResetOnChange", () => {
  it("does not run reset on the initial render", () => {
    const reset = vi.fn();
    function Probe({ value }: { value: string }) {
      useResetOnChange(value, reset);
      return null;
    }
    render(<Probe value="a" />);
    expect(reset).not.toHaveBeenCalled();
  });

  it("runs reset exactly once when the key changes, not on stable re-renders", () => {
    const reset = vi.fn();
    function Probe({ value }: { value: string }) {
      useResetOnChange(value, reset);
      return null;
    }
    const { rerender } = render(<Probe value="a" />);
    rerender(<Probe value="a" />); // stable key -> no reset
    expect(reset).not.toHaveBeenCalled();
    rerender(<Probe value="b" />); // changed -> one reset
    expect(reset).toHaveBeenCalledTimes(1);
    rerender(<Probe value="b" />); // stable again -> still one
    expect(reset).toHaveBeenCalledTimes(1);
    rerender(<Probe value="c" />); // changed -> two
    expect(reset).toHaveBeenCalledTimes(2);
  });

  it("applies the reset's setState before paint (no stale frame)", () => {
    const renders: Array<{ selected: string | null; list: string }> = [];
    function Probe({ list }: { list: string }) {
      const [selected, setSelected] = useState<string | null>("stale");
      // Clear the selection whenever the underlying list identity changes.
      useResetOnChange(list, () => setSelected(null));
      renders.push({ selected, list });
      return <span>{selected ?? "none"}</span>;
    }
    const { rerender, container } = render(<Probe list="v1" />);
    expect(container.textContent).toBe("stale");
    rerender(<Probe list="v2" />);
    // The committed (painted) output already reflects the reset, not "stale".
    expect(container.textContent).toBe("none");
  });

  it("compares by Object.is, so a new object reference triggers a reset", () => {
    const reset = vi.fn();
    function Probe({ value }: { value: { id: number } }) {
      useResetOnChange(value, reset);
      return null;
    }
    const same = { id: 1 };
    const { rerender } = render(<Probe value={same} />);
    rerender(<Probe value={same} />); // same reference -> no reset
    expect(reset).not.toHaveBeenCalled();
    rerender(<Probe value={{ id: 1 }} />); // equal but new reference -> reset
    expect(reset).toHaveBeenCalledTimes(1);
  });
});
