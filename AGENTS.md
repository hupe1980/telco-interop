# AGENTS.md

Instructions for coding agents working in this repository.

## What this is

The carrier interfaces of the German telecom market as sans-I/O Rust crates: `crates/wbci`
(WBCI 2.0), `crates/spri` (S/PRI 4.x), `crates/telco-interop` (both re-exported, plus
`Wechsel` across them), `crates/carrier-core` (what they share) and `crates/wita-kit` (name
reservation). One workspace, one crate per interface; shared code moves into `carrier-core`
only when two interfaces need it.

## Commands

| Goal | Command |
|---|---|
| Everything CI runs (except fuzzing) | `just ci` |
| Tests | `just test` |
| After an intended writer change | `just bless`, then review the fixture diff |
| Fuzz both readers | `just fuzz 60` |
| Site | `just site`, `just site-check` |
| Release | bump `version` and the `=x.y.z` pins in the root `Cargo.toml` (every crate inherits it), then `git tag -a vX.Y.Z -m vX.Y.Z && git push origin vX.Y.Z`; `.github/workflows/release.yml` publishes via crates.io Trusted Publishing |

## Rules

- No I/O, clock, environment or randomness in a library; time is an argument (`clippy.toml`
  forbids `now()`). The only exception is the `telco-interop` binary behind the `cli` feature,
  which reads files and prints; the library code it calls stays sans-I/O.
- Every rule the code enforces cites its source (document, version, clause) in its rustdoc
  or rule table; an interpretation says so, in the docs and on the site's scope page.
- Never commit or publish the AK S&P's documents, XSDs or WSDLs; fixtures are written here.
- Reading untrusted input never panics: no `unwrap`/`expect`/`panic!`/indexing outside tests.
- Personal data never appears in `Debug`, errors or validation issues.
- Errors and API prose are English; protocol terms stay German (`Vorabstimmung`, `EKPauf`).
- A user-visible change updates the crate README, `site/content/docs/` (its Rust samples run
  as doctests) and `CHANGELOG.md`.
- On the site, wrap every ```` ```rust ```` block in `{% <rust> %}` … `{% </rust> %}` (hides
  rustdoc's `# ` lines) and include each page with Rust code in its crate's `site_guides`
  module (`src/lib.rs`); `just site-check` enforces the wrapping.
- No planning ids (`FR-`, `SC-`, `SR-`, `T0…`) and no paths into the local planning trees in published
  files, except `<feature>/SR-###` evidence markers in tests — `just boundary` checks it.
