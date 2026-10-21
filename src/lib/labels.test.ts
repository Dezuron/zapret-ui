import { describe, expect, it } from "vitest";
import { formatBytes, formatTimestamp } from "./labels";

describe("formatBytes", () => {
  it("returns an empty string for zero", () => {
    expect(formatBytes(0)).toBe("");
  });

  it("formats kilobytes", () => {
    expect(formatBytes(2048)).toBe("2 КБ");
  });

  it("formats megabytes", () => {
    expect(formatBytes(2 * 1024 * 1024 + 1)).toBe("2.0 МБ");
  });
});

describe("formatTimestamp", () => {
  it("returns an empty string for zero", () => {
    expect(formatTimestamp(0)).toBe("");
  });

  it("formats a unix timestamp", () => {
    expect(formatTimestamp(1_700_000_000)).toContain("2023");
  });
});
