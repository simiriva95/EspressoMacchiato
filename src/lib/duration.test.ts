import { describe, expect, it } from "vitest";
import { secondsUntil, toDurationSecs } from "./duration";

const at = (h: number, m: number) => {
  const d = new Date(2026, 7, 4); // clock date is irrelevant, wall time only
  d.setHours(h, m, 0, 0);
  return d;
};

describe("secondsUntil", () => {
  it("computes seconds to a later time today", () => {
    expect(secondsUntil("18:00", at(17, 0))).toBe(3600);
  });

  it("wraps to tomorrow when already past", () => {
    expect(secondsUntil("18:00", at(18, 30))).toBe(86_400 - 1800);
  });

  it("rejects invalid input", () => {
    expect(secondsUntil("25:00", at(12, 0))).toBeNull();
    expect(secondsUntil("6pm", at(12, 0))).toBeNull();
  });
});

describe("toDurationSecs", () => {
  it("maps the three choice kinds", () => {
    expect(toDurationSecs({ kind: "indefinite" })).toBeUndefined();
    expect(toDurationSecs({ kind: "minutes", minutes: 30 })).toBe(1800);
    expect(toDurationSecs({ kind: "until", time: "13:00" }, at(12, 0))).toBe(
      3600,
    );
  });
});
