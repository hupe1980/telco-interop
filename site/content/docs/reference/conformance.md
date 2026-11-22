+++
title = "Schema conformance"
description = "How the WBCI and S/PRI writers are proven against the official AK S&P schemas on every CI run: 373 WBCI and 776 S/PRI payloads, schemas pinned by SHA-256."
weight = 3

[extra]
nav = "Schema conformance"
seo_title = "Schema conformance: WBCI XSD V2.00 and S/PRI XSD 4.00.03 · telco-interop"
+++

Round-trip tests prove that the crates read what they write. They do not prove that a peer
accepts it. So every CI run validates what the writers produce against the AK S&P's
published schemas with `xmllint`.

## The corpora

| | WBCI | S/PRI |
|---|---|---|
| Schema | Anlage 4 XSD V2.00 | XSD 4.00.03 (all 4.x) |
| Payloads | **373** | **776** |
| Every request, order and message kind | 15 fixtures | 30 fixtures |
| Every published code on every message kind and business case it belongs to | 105 | 618 |
| The schema's maxima (WBCI: 99 positions of 10 numbers, 99 further holders, 99 resources and lines, 5 PBX blocks) | 5 | 5 |
| Maximum-length texts with umlauts and `<&>"'` | 2 | 3 |
| The answers of the [losing provider's check](@/docs/wbci/pruefung.md) | 6 | — |
| Generated variants | 240 | 120 |

The [sandbox](@/docs/across-both/sandbox.md) scenarios are validated too: every payload of every
scenario, played from two start times — 64 WBCI and 248 S/PRI payloads.

A negative control proves the validator rejects a misordered element, so a silently passing
`xmllint` cannot hide a broken test.

## The schemas are not redistributed

The schemas are the AK S&P's copyrighted documents. `scripts/fetch-wbci-xsd.sh` and
`scripts/fetch-spri-xsd.sh` download them from ak-spri.de and accept them only by their pinned
SHA-256; every CI run notices when a new version is published.

## Run it yourself

```sh
just schema    # fetch both schemas, then validate both corpora and the scenarios (needs xmllint)
```

Passing the schema is necessary, not sufficient: the [validation rules](@/docs/wbci/validation.md)
and the [processes](@/docs/wbci/process.md) cover what a schema cannot express, and
certification with your peers remains your step.
