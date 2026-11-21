+++
title = "WBCI XML and SOAP"
description = "Read and write the four WBCI payloads, bare or in a SOAP 1.1 envelope, byte-stable, with limits for untrusted input and a receipt code for every error."
weight = 2

[extra]
nav = "XML and SOAP"
+++

## The four payloads

| `Payload` | Element | Carries |
|---|---|---|
| `AnfrageRequest` | `annehmenAnfrageRequest` | `Control`, `Anfrage` |
| `AnfrageResponse` | `annehmenAnfrageResponse` | `Control`, `Quittung` (TEQ) |
| `MeldungRequest` | `annehmenMeldungRequest` | `Control`, `Meldung` |
| `MeldungResponse` | `annehmenMeldungResponse` | `Control`, `Quittung` (TEQ) |

Only these four elements are namespace-qualified (`http://www.vatm.de/wbci/v1/envelope`);
the schemas set no `elementFormDefault`, so everything inside is unqualified. The abstract
types carry `xsi:type` (`ar:` for requests, `am:` for messages and positions). The writer
emits the schema's element order with two-space indentation; reading what it wrote and writing
again gives the same bytes.

`Control` carries the timestamp (with its UTC offset, which is kept), release `02`/`00` and
the signing certificate's issuer and serial. `Control::new` fills in the release.

## SOAP

`Payload::to_soap` wraps the payload in a SOAP 1.1 envelope with an empty `Header` for your
WS-Security signer. `Payload::from_xml` accepts the bare payload or an envelope, skipping the
header. Signing, TLS and HTTP stay in your application.

## Untrusted input

{% <rust> %}
```rust
use wbci::xml::{Limits, ReadErrorKind};
use wbci::{Meldungscode, Payload};

let bomb = r#"<!DOCTYPE x [<!ENTITY a "aaaa">]><x>&a;</x>"#;
let err = Payload::from_xml(bomb.as_bytes()).unwrap_err();
assert_eq!(err.kind, ReadErrorKind::Doctype);
assert_eq!(err.teq_code(), Meldungscode::DatenstrukturUngueltig); // answer with TEQ 0995

let strict = Limits { max_bytes: 64 * 1024, ..Limits::default() };
assert!(Payload::from_xml_with(b"<x/>", &strict).is_err());
```
{% </rust> %}

| Refused | Receipt code |
|---|---|
| a `DOCTYPE`, any entity other than `lt gt amp apos quot` | 0995 |
| input over `max_bytes` (1 MiB), deeper than `max_depth` (32), more than `max_elements` (10 000), text over `max_text` (4 096) | 0995 |
| a repetition over the schema's maximum (99 positions, 10 numbers, 5 blocks) | 0995 |
| an unknown or wrongly namespaced `xsi:type`, a missing or unexpected element, an invalid value | 0995 |
| a major release other than `02` | 0993 |

A newer minor release is accepted: minor versions are compatible by WBCI 2.0 §11.4. Every
error names the element path and byte offset, never the content.

## Proven against the official schema

Every payload kind the writer produces is validated against the AK S&P's XML schema (Anlage 4
V2.00) on each CI run, 373 payloads in all. [Schema conformance](@/docs/reference/conformance.md)
shows what the corpus contains and how to run it yourself; [security](@/docs/reference/security.md)
lists every guarantee for untrusted input.
