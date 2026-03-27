# Rustls Provider Tests

This crate is an unpublished workspace crate that holds integration tests for different cryptography providers
and associated machinery. We add tests here to avoid taking heavy dependencies on the main rustls crate.

## Reduced test vectors

The file `tests/rfc-9180-test-vectors.json` has been filtered from the original upstream version:

- **Original:** 128 vectors (4 HPKE modes × 32 suite combinations) — 5.8 MB
- **Filtered:** 32 vectors (mode=0 base mode only) — 1.4 MB
- **Reduction:** 75%

Vectors for modes 1 (PSK), 2 (Auth), and 3 (Auth+PSK) were removed because rustls only implements HPKE base mode (mode 0). The `rfc_tests::applicable()` function already skips these modes (`if self.mode != 0 { return None; }`), so the filtered vectors were never exercised. All applicable test vectors are preserved.
