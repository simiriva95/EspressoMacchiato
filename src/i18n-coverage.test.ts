// @vitest-environment node
// Guard rail from spec §10: no hardcoded visible strings in components.
// Scans JSX text nodes; anything with letters must come from t(...).

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import enJson from "./i18n/en.json";
import itJson from "./i18n/it.json";

const SRC = fileURLToPath(new URL(".", import.meta.url));

function tsxFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name: string) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return tsxFiles(path);
    if (path.endsWith(".tsx") && !path.includes(".test.")) return [path];
    return [];
  });
}

// Literal JSX text between tags containing at least two letters.
const JSX_TEXT = />\s*([^<>{}]*[A-Za-zÀ-ù]{2,}[^<>{}]*)</g;
const ALLOWED = new Set(["EspressoMacchiato", "Italiano", "English"]);

describe("i18n coverage", () => {
  it("components contain no hardcoded visible strings", () => {
    const offenders: string[] = [];
    for (const file of tsxFiles(SRC)) {
      const source = readFileSync(file, "utf8");
      for (const match of source.matchAll(JSX_TEXT)) {
        const text = match[1].trim();
        // Code fragments the regex can catch (TS generics, expressions)
        // are never visible copy: visible JSX text has no such tokens.
        if (/[;=()"'`&|]/.test(text)) continue;
        if (text && !ALLOWED.has(text)) {
          offenders.push(`${file}: "${text}"`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  it("it.json and en.json expose the same keys", () => {
    const keys = (obj: object, prefix = ""): string[] =>
      Object.entries(obj).flatMap(([k, v]) =>
        typeof v === "object" && v !== null
          ? keys(v, `${prefix}${k}.`)
          : [`${prefix}${k}`],
      );
    expect(keys(itJson).sort()).toEqual(keys(enJson).sort());
  });
});
