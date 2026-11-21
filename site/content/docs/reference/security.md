+++
title = "Security and hardening"
description = "What the WBCI and S/PRI readers guarantee for XML from other companies: no DTDs, bounded input, no panics (fuzzed), no unsafe, personal data redacted."
weight = 2

[extra]
nav = "Security and hardening"
seo_title = "Security: hardened XML reading for WBCI and S/PRI · telco-interop"
+++

The crates parse XML that other companies send. A bug in reading it is a security bug, so the
reader both interfaces share is built for hostile input.

## Reading untrusted XML

| Guarantee | How |
|---|---|
| No DTD, no entity expansion | a `DOCTYPE` is refused; only the five predefined entities (`lt gt amp apos quot`) are resolved, so no billion-laughs, no external entities |
| Bounded input | `xml::Limits`: input size (1 MiB), nesting depth (32), element count (10 000), text per element (4 096 characters), S/PRI attachment size (1 MiB decoded) |
| Bounded repetitions | every list is capped at the schema's maximum, or a reader bound where the schema has none |
| A receipt code for every error | `ReadError::teq_code()`: 0993 for a wrong major release, 0995 for everything else |
| No values in errors | an error names the element path and byte offset, never the content |
| Never a panic | no `unwrap`, `expect`, `panic!` or indexing outside tests (Clippy denies them); two fuzz targets run on every push (2 minutes each) |
| No `unsafe` | `unsafe_code = "forbid"` across the workspace |

The [WBCI XML guide](@/docs/wbci/xml.md#untrusted-input) shows the limits in code and lists
which input is refused with which receipt code.

## Personal data

Names, street, house number, phone numbers, e-mail addresses, free text and attachment content
print redacted in `Debug` (`Nachname(<redacted, 10 chars>)`), so logging a value does not log
the customer; `Display` and `as_str()` give the value when you need it. Errors, validation
issues, the losing provider's findings, the switch timeline and its findings carry ids, dates,
message types and codes only.

## Attachments

An S/PRI attachment's file name comes from another company. `Anlage::safe_file_name()` gives a
name without path separators or a leading dot; never join `dateiname` to a folder as it is
(see [S/PRI attachments](@/docs/spri/orders-and-messages.md#attachments)).

## Supply chain

Three external dependencies (`quick-xml`, `jiff`, `thiserror`), checked with `cargo deny` in
CI; a test fails when a dependency is added without review. The time-zone rules for
`Europe/Berlin` are compiled in, so no system database is read at run time. No package ships
an AK S&P document: CI checks every package's file list.

## What stays yours

The crates perform no I/O. TLS, WS-Security signing and verification, IP allow-listing,
authentication of the peer, rate limiting and storage are your application's job.

## Reporting a vulnerability

Report privately with **Report a vulnerability** under
[GitHub Security](https://github.com/hupe1980/telco-interop/security), not in a public issue.
Use synthetic data only: never real customer data, carrier certificates or production payloads.
