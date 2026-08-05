// M3 DoD: zero axe violations on the two windows. The ipc module serves
// demo data outside Tauri, so both windows render fully in jsdom.
// color-contrast is checked by hand in tokens.css (jsdom has no canvas).

import { render } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it } from "vitest";
import { Dashboard } from "./windows/Dashboard";
import { Hud } from "./windows/Hud";

async function expectNoViolations(container: HTMLElement) {
  const results = await axe.run(container, {
    rules: { "color-contrast": { enabled: false } },
  });
  const summary = results.violations.map(
    (v) => `${v.id}: ${v.nodes.map((n) => n.html).join(" | ")}`,
  );
  expect(summary).toEqual([]);
}

describe("accessibility (axe)", () => {
  it("Dashboard has no violations", async () => {
    const { container, findByTestId } = render(<Dashboard />);
    await findByTestId("idle-value"); // let async state settle
    await expectNoViolations(container);
  }, 15000);

  it("HUD has no violations", async () => {
    const { container, findByTestId } = render(<Hud />);
    await findByTestId("idle-value");
    await expectNoViolations(container);
  }, 15000);
});
