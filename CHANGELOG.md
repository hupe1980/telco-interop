# Changelog

All notable changes to the crates in this repository. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer, with
breaking changes allowed before 1.0.

## wbci 0.1.0 — 2026-11-21

WBCI 2.0 (AK S&P; XSD V2.00, Anlage 1 V5.4.1, Arbeitshandbuch WBCI 2.10).

### Added

- Typed values for every constrained schema type; errors name the facet and position, never
  the value. `VorabstimmungsId`/`StornoId`/`AenderungsId::allocate` from a caller sequence.
- All six requests and six messages; request senders derived from the request type; answer
  constructors (`RuemVa::zustimmung`, `Abbm::ablehnung`, `Erlm::storno_ok`,
  `Erlm::terminverschiebung_ok`, `AkmTr::uebernahme`, `AbbmTr::ablehnung`, `Teq::for_*`).
- XML read and write of the four payloads, bare or in a SOAP 1.1 envelope, byte-stable.
  Hardened reader: DTDs refused, only predefined entities, bounded size, depth, element count,
  text and repetitions; every read error maps to its receipt code (0993/0995).
- The 36 Meldungscodes with text, short name, message kinds and business cases.
- Validation with 28 cited rules (`Anfrage::issues`, `issues_at`, `Meldung::issues`,
  `issues_at`, `Teq::issues`).
- Working-day calendar (nationwide holidays, Easter computed) and explained deadlines for
  every rule of Anlage 1 §2.5–§2.11, in `Europe/Berlin` (compiled in).
- `Process`: state machine per pre-agreement with role checks, 8115/8118/8140/8141/8142/8148
  refusals, negative-receipt rollback, next actions, open and overdue deadlines, history.
- `pruefung`: the losing provider's check (Anl. 1 V5.4.1 §2.3/§2.4, AK S&P Infodokument
  „Phonetische Prüfung" v1.1, Prüfbaum V1.1). `check` decides the RUEM-VA (ZWA/NAT, ADA,
  resources, ported numbers) or the ABBM with every reason (RNG, ADF, AIF, AIFVN, WAI, KNI,
  VAE, NWE, SON), explained per field without personal data; `not_found` (KNI);
  `koelner_phonetik` and `same_name` for your own search.
- Personal data redacted in `Debug`.
- Conformance: 373 payloads validated against the official XSD V2.00 in CI (`just schema`);
  the schema is fetched and pinned by SHA-256, never committed.

## spri 0.1.0 — 2026-11-21

S/PRI 4.x (AK S&P; schema 4.00.03, Providervereinbarung V4.4, Anlage 7 V.13, Arbeitshandbuch
V2.9.5).

### Added

- The typed model: all eight order business cases (NEU, KUE-AG, LAE, AEN-LMAE, EST, PV, GET,
  SET) with FTTx products, appointments, contacts and attachments; all twenty message types
  and the TEQ; the four payloads, bare or in SOAP 1.1.
- The 215 message codes of the Arbeitshandbuch with text, message types and business cases;
  unknown codes kept as `Other`; `ReadError::teq_code`.
- `Process`: the order lifecycle of Anlage 7 (EPKs V.13) for every business case, Storno and
  Terminverschiebung, the PV leg with the losing provider (`from_akm_pv`), KDLE (`from_kdle`),
  TAM/MTAM/ERLM-K; refusals name the ABBM to answer with; next actions, deadlines
  (`spri::deadline`), history. `Abbm::outcome` (resend, continues, closed).
- Hardened reader (shared with wbci), attachments bounded (1 MiB default); personal and
  contact data redacted in `Debug`; `Anlage::safe_file_name()`.
- Conformance: 776 payloads validated against the official XSD 4.00.03 in CI, pinned by
  SHA-256, never committed.

## telco-interop 0.1.0 — 2026-11-21

`wbci` and `spri` re-exported behind features (default on), plus the identifiers they share:
one dependency and one version for a gateway speaking both.

### Added

- `Wechsel`: one provider switch across WBCI and S/PRI — `attach` by Vorabstimmungs-ID
  (refused processes handed back), `timeline`, `befunde` (six cross-interface rules),
  `deadlines`, `next_actions` per party.
- The `telco-interop` command (feature `cli`, not default): `read`, `check`, `code`,
  `timeline --as ekpauf|ekpabg|netzbetreiber`. No personal data in its output, control
  characters neutralised, bounded folder reads; exit 1 on errors, findings or overdue
  obligations, 2 on usage errors. Install with `brew install hupe1980/tap/telco-interop`, a
  pre-built binary from the release, or `cargo install telco-interop --features cli`.

## carrier-core 0.1.0 — 2026-11-21

### Added

- The foundation `wbci` and `spri` share: value facets and `ValueError`, `CarrierCode`,
  `VorabstimmungsId`, `LineId`, the working-day calendar (`Calendar::wbci`,
  `Calendar::werktage`) and explained counting, `Zeitstempel`, `SignaturId`, the bounded XML
  reader and writer. Used through `wbci` and `spri`.

## spri 0.0.1, wita-kit 0.0.1, wbci 0.0.1

Name reservations.
