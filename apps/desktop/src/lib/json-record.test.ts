import { describe, expect, it } from "vitest";

import { coerceJsonRecord, parseJsonRecord, parseNumberRecord } from "./json-record";

describe("parseJsonRecord", () => {
  it("returns an object record for valid object JSON", () => {
    expect(parseJsonRecord('{"remaining_updates":2,"source":"updates"}')).toEqual({
      remaining_updates: 2,
      source: "updates",
    });
  });

  it("rejects arrays, null, and primitive JSON values", () => {
    expect(parseJsonRecord('[["remaining_updates",2]]')).toBeNull();
    expect(parseJsonRecord("null")).toBeNull();
    expect(parseJsonRecord('"detail"')).toBeNull();
    expect(parseJsonRecord("7")).toBeNull();
  });

  it("returns null for malformed JSON", () => {
    expect(parseJsonRecord("{bad json")).toBeNull();
  });
});

describe("parseNumberRecord", () => {
  it("accepts records with finite number values", () => {
    expect(parseNumberRecord({ "7:src/App.tsx": 1_714_000_000_000 })).toEqual({
      "7:src/App.tsx": 1_714_000_000_000,
    });
  });

  it("rejects records with non-number or non-finite values", () => {
    expect(parseNumberRecord({ key: "123" })).toBeNull();
    expect(parseNumberRecord({ key: Number.NaN })).toBeNull();
    expect(parseNumberRecord(["not", "a", "record"])).toBeNull();
  });
});

describe("coerceJsonRecord", () => {
  it("accepts object values and JSON-encoded object values", () => {
    const record = { days_until_expiry: 12 };
    expect(coerceJsonRecord(record)).toBe(record);
    expect(coerceJsonRecord('{"days_until_expiry":12}')).toEqual(record);
  });

  it("rejects malformed JSON and non-record values", () => {
    expect(coerceJsonRecord("{bad json")).toBeNull();
    expect(coerceJsonRecord(["days_until_expiry", 12])).toBeNull();
    expect(coerceJsonRecord(null)).toBeNull();
    expect(coerceJsonRecord(12)).toBeNull();
  });
});
