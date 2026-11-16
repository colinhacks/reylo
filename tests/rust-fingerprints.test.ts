// The Rust crate reads the cache the loader writes (rust/reylo-codegen/src/analysis.rs), whose
// entries are current while the schema's fingerprint is. Each fingerprint query it runs is a copy
// of the loader's, so a change to one that the other misses would leave stale types looking current.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { FINGERPRINT } from "../src/analyze.ts";

for (const dialect of ["postgres", "mysql", "sqlite"] as const) {
  test(`the Rust crate's ${dialect} schema fingerprint is the loader's`, () => {
    const copy = readFileSync(new URL(`../rust/reylo-codegen/src/fingerprint-${dialect}.sql`, import.meta.url), "utf8");
    assert.equal(copy, FINGERPRINT[dialect], `rust/reylo-codegen/src/fingerprint-${dialect}.sql differs from FINGERPRINT.${dialect} in src/analyze.ts: copy it over`);
  });
}
