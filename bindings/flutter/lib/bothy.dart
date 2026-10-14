/// Bothy: serverless, local-first sync over the user's own Google Drive.
///
/// Early development. This release only contains the on-remote format version and the
/// hybrid logical clock (HLC) timestamp type. The sync engine will ship as bindings to the
/// Rust core: https://github.com/bothy-sync/bothy
library;

export 'src/hlc.dart';

/// Version of the on-remote file format (`bothy_format` in `manifest.json`). Draft.
const int bothyFormatVersion = 1;
