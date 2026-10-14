/** Version of the on-remote file format (`bothy_format` in `manifest.json`). Draft. */
export declare const FORMAT_VERSION: 1;

/** Hybrid logical clock timestamp: 48-bit physical ms + 16-bit logical counter, as a 64-bit bigint. */
export type Hlc = bigint;

export declare function hlcFromParts(physicalMs: number | bigint, logical: number): Hlc;
export declare function hlcToParts(hlc: Hlc): { physicalMs: number; logical: number };
export declare function parseHlc(text: string): Hlc;
export declare function formatHlc(hlc: Hlc): string;

/** Key derivation parameters for an encrypted database (draft). */
export interface KdfParams {
  alg: "argon2id";
  m_kib: number;
  t: number;
  p: number;
  /** Base64, 16 bytes. */
  salt: string;
}

/** Encryption block of `manifest.json` (draft). */
export interface ManifestEncryption {
  alg: "xchacha20poly1305";
  kdf: KdfParams;
  /** Base64: nonce (24) || ciphertext (32) || tag (16). */
  wrapped_dek: string;
  key_check: string;
}

/** `manifest.json` at the root of a Bothy database (draft; see SPEC.md). */
export interface Manifest {
  bothy_format: number;
  /** UUID of the database. */
  db_id: string;
  /** 16-character hex HLC. */
  created_hlc: string;
  /** 32-character hex device id. */
  created_by: string;
  app_id: string;
  app_schema_version: number;
  codec: { serialization: "cbor"; compression: "zstd" | "none" };
  encryption: ManifestEncryption | null;
  compaction: { grace_period_days: number };
}
