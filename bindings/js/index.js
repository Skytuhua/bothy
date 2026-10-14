/** Version of the on-remote file format (`bothy_format` in `manifest.json`). Draft. */
export const FORMAT_VERSION = 1;

const LOGICAL_BITS = 16n;
const MAX_PHYSICAL_MS = (1n << 48n) - 1n;
const HEX_RE = /^[0-9a-f]{16}$/;

/** Builds an HLC (48-bit physical ms + 16-bit logical counter) as a 64-bit bigint. */
export function hlcFromParts(physicalMs, logical) {
  const p = BigInt(physicalMs);
  const l = BigInt(logical);
  if (p < 0n || p > MAX_PHYSICAL_MS) throw new RangeError("physicalMs must fit in 48 bits");
  if (l < 0n || l > 0xffffn) throw new RangeError("logical must fit in 16 bits");
  return (p << LOGICAL_BITS) | l;
}

/** Splits a 64-bit HLC into its parts. `physicalMs` fits in a JS number (48 bits). */
export function hlcToParts(hlc) {
  const h = BigInt.asUintN(64, BigInt(hlc));
  return { physicalMs: Number(h >> LOGICAL_BITS), logical: Number(h & 0xffffn) };
}

/** Parses the fixed-width, lowercase 16-character hex form used in file names. */
export function parseHlc(text) {
  if (typeof text !== "string" || !HEX_RE.test(text)) {
    throw new SyntaxError("HLC must be exactly 16 lowercase hex characters");
  }
  return BigInt("0x" + text);
}

/** Formats an HLC in its fixed-width hex form; sorts like the timestamp itself. */
export function formatHlc(hlc) {
  return BigInt.asUintN(64, BigInt(hlc)).toString(16).padStart(16, "0");
}
