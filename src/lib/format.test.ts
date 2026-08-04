import { describe, expect, it } from "vitest";
import { formatClock, formatSeconds } from "./format";

describe("formatSeconds", () => {
  it("formats sub-minute values", () => {
    expect(formatSeconds(0)).toBe("0s");
    expect(formatSeconds(45.7)).toBe("45s");
    expect(formatSeconds(59)).toBe("59s");
  });

  it("formats minutes with padded seconds", () => {
    expect(formatSeconds(60)).toBe("1m 00s");
    expect(formatSeconds(65)).toBe("1m 05s");
    expect(formatSeconds(3599)).toBe("59m 59s");
  });

  it("formats hours", () => {
    expect(formatSeconds(3600)).toBe("1h 00m");
    expect(formatSeconds(3720)).toBe("1h 02m");
  });

  it("clamps negatives to zero", () => {
    expect(formatSeconds(-5)).toBe("0s");
  });
});

describe("formatClock", () => {
  it("dashes when absent", () => {
    expect(formatClock(null)).toBe("—");
  });

  it("renders a time for a timestamp", () => {
    expect(formatClock(Date.UTC(2026, 0, 1, 12, 0, 0))).toMatch(/\d/);
  });
});
