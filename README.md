# telco-interop

**The German carrier interfaces as Rust crates: WBCI for the § 59 TKG provider switch, S/PRI
for open-access fibre orders.**

[![CI](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml/badge.svg)](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/telco-interop.svg)](https://crates.io/crates/telco-interop)
[![docs.rs](https://docs.rs/telco-interop/badge.svg)](https://docs.rs/telco-interop)
![MSRV 1.88](https://img.shields.io/badge/rustc-1.88+-orange.svg)
![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)

When a customer in Germany switches provider (*Anbieterwechsel*), the gaining and the losing
provider agree the switch over **WBCI**, and on open-access networks the gaining provider
orders the line from the network owner over **S/PRI**; the AK S&P specifies both. These
crates give you typed messages, XML proven against the official schemas, explained deadlines
and the process of each switch and order as a value. They are sans-I/O: transport, signing
and storage stay in your application.

```rust
use wbci::pruefung::{self, Bestand};
use wbci::*;

// `xml` and `received` come from your SOAP endpoint: a pre-agreement from the gaining provider.
let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(&xml)? else {
    return Err("not a pre-agreement".into());
};

// As the losing provider: decide the answer from your record.
let mut bestand = Bestand::new(va.endkunde.clone(), "D123".parse()?);
bestand.rufnummern = vec![OnkzRufnummer { onkz: "30".parse()?, rufnummer: "1234567".parse()? }];
bestand.standort = va.standort().cloned();
let pruefung = pruefung::check(&va, &bestand, received, &Calendar::wbci());
assert!(pruefung.is_agreed()); // RUEM-VA, every field explained

// Follow the switch: what is due, by whom, and why.
let mut process = Process::new(Role::EkpAbg, va.clone(), Direction::Received, received)?;
process.apply(Direction::Sent, received, Teq::for_anfrage(&va.into(), Meldungscode::Ok))?;
println!("{}", process.deadlines()[0]); // EKPabg: by Do 26.11.2026 23:59 — … (Anl. 1 V5.4.1 §2.5)
```

## Crates

| Crate | Interface | Between | Status |
|---|---|---|---|
| [`wbci`](crates/wbci) | WBCI 2.0 | provider ↔ provider: the § 59 TKG switch, number porting, the losing provider's check | complete |
| [`spri`](crates/spri) | S/PRI 4.x | service provider ↔ network owner: open-access orders, faults, diagnostics, the provider switch | complete |
| [`telco-interop`](crates/telco-interop) | both | one dependency for both, plus `Wechsel`: one switch across WBCI and S/PRI, the sandbox scenarios (`sim`), the `cli` command line and its output as a library (`report`) | complete |
| [`carrier-core`](crates/carrier-core) | — | the shared foundation: ids, working-day calendar, hardened XML; used through the crates above | internal |
| [`wita-kit`](crates/wita-kit) | WITA | service provider ↔ Deutsche Telekom | name reserved |

## Highlights

| | |
|---|---|
| **Schema-proven** | every payload kind validated against the official XSDs on each CI run: 373 WBCI, 776 S/PRI payloads |
| **Know before you send** | 28 cited WBCI and 34 S/PRI rules predict the peer's refusal and its code |
| **Explained deadlines** | Arbeitstage and Werktage in German time, with every counted and skipped day |
| **Processes** | a state machine per WBCI pre-agreement and per S/PRI order (Anlage 7 EPKs) |
| **Across both** | `Wechsel`: one timeline, both interfaces' deadlines, six agreement checks |
| **Sandbox** | simulated peers (`wbci::sim`, `spri::sim`) and 13 scenarios that play whole switches and orders from both sides, every message valid and on time: test your side without a partner |
| **Command line** | read, check, explain codes, replay a switch, write a scenario's files (`simulate`): `brew install hupe1980/tap/telco-interop` or `cargo install telco-interop --features cli` |
| **Playground** | [read, check and explain a payload, or show a sandbox scenario, in the browser](https://hupe1980.github.io/telco-interop/playground/): WebAssembly, nothing uploaded |
| **Hardened** | no DTDs, bounded input, fuzzed, no panics, no `unsafe`, personal data redacted |

## Documentation

- [Guides](https://hupe1980.github.io/telco-interop/): [how a switch works](https://hupe1980.github.io/telco-interop/docs/start/introduction/),
  [getting started](https://hupe1980.github.io/telco-interop/docs/start/getting-started/),
  [glossary](https://hupe1980.github.io/telco-interop/docs/start/glossary/),
  [command line](https://hupe1980.github.io/telco-interop/docs/start/command-line/),
  [sandbox](https://hupe1980.github.io/telco-interop/docs/across-both/sandbox/),
  [playground](https://hupe1980.github.io/telco-interop/playground/)
- API: [telco-interop](https://docs.rs/telco-interop), [wbci](https://docs.rs/wbci), [spri](https://docs.rs/spri)
- Example: `cargo run -p wbci --example switch`

## Development

```sh
just ci          # fmt, boundary, clippy, rustdoc, tests, MSRV, cargo-deny, package and site checks
just example     # the switch end to end
just schema      # validate both writers and the sandbox scenarios against the official XSDs (fetched, pinned by SHA-256)
just fuzz 60     # fuzz both readers (nightly + cargo-fuzz)
just bless       # regenerate the golden XML fixtures after an intended change
just playground  # build the site's playground module (needs the wasm32-unknown-unknown target)
just site        # serve the documentation site (Zola), with the playground when the target is installed
```

See [AGENTS.md](AGENTS.md) for the rules of the codebase and [SECURITY.md](SECURITY.md) for
reporting vulnerabilities.

## Scope

Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.
The specifications are their copyrighted documents: this repository carries its own types and
synthetic fixtures, never the documents. [Scope and interpretations](https://hupe1980.github.io/telco-interop/docs/reference/scope/)
lists the implemented versions and every interpretation.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
