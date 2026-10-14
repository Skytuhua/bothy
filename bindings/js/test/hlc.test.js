import { test } from "node:test";
import assert from "node:assert/strict";
import { FORMAT_VERSION, formatHlc, hlcFromParts, hlcToParts, parseHlc } from "../index.js";

const T = 1_700_000_000_000;

test("matches the Rust test vector", () => {
  assert.equal(formatHlc(hlcFromParts(T, 1)), "018bcfe568000001");
  assert.deepEqual(hlcToParts(parseHlc("018bcfe568000001")), { physicalMs: T, logical: 1 });
});

test("hex ordering matches numeric ordering", () => {
  const a = hlcFromParts(T, 1);
  const b = hlcFromParts(T + 1, 0);
  assert.ok(a < b);
  assert.ok(formatHlc(a) < formatHlc(b));
});

test("rejects malformed hex", () => {
  for (const s of ["", "018bcfe56800000", "018bcfe5680000011", "018BCFE568000001", "018bcfe56800000g"]) {
    assert.throws(() => parseHlc(s), SyntaxError, s);
  }
});

test("rejects out-of-range parts", () => {
  assert.throws(() => hlcFromParts(2 ** 48, 0), RangeError);
  assert.throws(() => hlcFromParts(T, 65536), RangeError);
});

test("format version", () => {
  assert.equal(FORMAT_VERSION, 1);
});
