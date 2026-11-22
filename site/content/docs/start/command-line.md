+++
title = "Command line"
description = "The telco-interop command: read and check WBCI and S/PRI files, explain a Meldungscode, replay one provider switch as a timeline with findings and deadlines."
weight = 4

[extra]
seo_title = "WBCI and S/PRI on the command line · telco-interop"
+++

The `telco-interop` command puts the library in front of files: what a message is, whether it
is valid, what a code means and where a switch stands. It reads only the files you name and
prints no personal data — names, addresses, numbers and free text never appear in its output.

To try `read`, `check` and `code` without installing anything, use the
[playground](@/playground.md): the same lines, computed in your browser. Both print through
[`telco_interop::report`](https://docs.rs/telco-interop/latest/telco_interop/report/) (feature
`report`), which gives those lines from bytes to any other front end.

## Install

### Homebrew

```sh
brew install hupe1980/tap/telco-interop
```

macOS on Apple Silicon and Linux (x86_64, arm64). `brew upgrade telco-interop` moves to a new
release.

### Pre-built binary

Every [release](https://github.com/hupe1980/telco-interop/releases) carries one archive per
platform — Linux (x86_64, arm64, glibc or static musl), macOS (Apple Silicon), Windows — and a
`SHA256SUMS` file to check them against.

### From source

```sh
cargo install telco-interop --features cli
```

The command line is the `cli` feature of the [`telco-interop`](https://docs.rs/telco-interop)
crate. It is not on by default, so the library never pulls in the argument parser.

## Read a file

`read` names the interface, the kind, the business case, the ids, dates and codes of each file —
or why it cannot be read, with the TEQ code to answer with:

```text
$ telco-interop read abbm.xml
abbm.xml
  interface: WBCI
  payload: message (annehmenMeldungRequest)
  kind: ABBM
  business case: VA-KUE-MRN
  sent: 2026-11-23T09:30:00+01:00
  sender: DEU.EKP2 (EKPabg)
  EKPauf: DEU.EKP1
  EKPabg: DEU.EKP2
  Vorabstimmungs-ID: DEU.EKP1.V000000013
  code: 8103 Rufnummer nicht geschaltet (RNG)
```

Files may be bare payloads or SOAP 1.1 envelopes; the interface is detected from the content.

## Check a file

`check` lists the validation issues of each [WBCI](@/docs/wbci/validation.md) or
[S/PRI](@/docs/spri/validation.md) file — request, order, message or receipt — with rule id,
source and the code the peer would answer with, or why the file cannot be read. The date rules
of requests and orders (lead times, a date in the past) are judged at the file's own control
timestamp, the moment it was sent:

```text
$ telco-interop check storno.xml
storno.xml
  warning [str-auf.ekpabg-grund] Anfrage/Stornogrund: an EKPabg may cancel only for the reasons of Anl. 1 §2.9.2 (…) and must hold proof (Anl. 1 V5.4.1 §2.9.2)
```

```text
$ telco-interop check tv.xml
tv.xml
  error [spri.aekz.terminverschiebung] auftrag/aenderungskennzeichen: AEN-LMAE has no Terminverschiebung; only NEU, KUE-AG, LAE and PV have one (S/PRI 4.4 §5.3.1.3, §6.1.3, §6.2.1.3, §6.2.2.3) → 1007
```

`check` exits 1 when a file is unreadable or has an error; warnings alone exit 0. It checks each
file on its own; a follow-up order against its original is the library call
`Auftrag::issues_against`.

## Explain a code

```text
$ telco-interop code 8103
WBCI 8103 Rufnummer nicht geschaltet (RNG)
  short name: RNG
  ABBM in VA-KUE-MRN, VA-KUE-ORN, VA-RRNP
S/PRI: 8103 is not an S/PRI code
```

`--wbci` or `--spri` limits the lookup to one interface. The full lists are on the
[WBCI codes](@/docs/wbci/codes.md) and [S/PRI codes](@/docs/spri/codes.md) pages.

## Replay a switch

`timeline` reads the files of one switch — WBCI and S/PRI, files or folders — and replays them
from one party's view, as [one switch](@/docs/across-both/one-switch.md) does in code:

```text
$ telco-interop timeline --as ekpauf switch/
Switch DEU.EKP1.V000000013, replayed as EKPauf
Now: 2026-11-27T10:00:05Z (the latest control timestamp)
Timeline:
  Mo 23.11.2026 09:00 WBCI  EKPauf → VA-KUE-MRN
  Di 24.11.2026 10:00 WBCI  EKPabg → RUEM-VA 8001 8104
  Do 26.11.2026 09:00 S/PRI EKPauf → PV
  Fr 27.11.2026 11:00 S/PRI Netzbetreiber → ABM 0000 0002
  …  (the TEQ receipts omitted)
Findings: none
Deadlines:
  open  WBCI EKPauf: by Mo 11.01.2027 23:59 — A cancellation (Aufhebung) arrives at least 3 AT … (Anl. 1 V5.4.1 §2.9.1, §2.9.2)
```

| Option | Meaning |
|---|---|
| `--as ekpauf` | the gaining provider (WBCI EKPauf, S/PRI Auftraggeber) |
| `--as ekpabg` | the losing provider (WBCI EKPabg, S/PRI abgebender Provider): its WBCI and the PV leg it takes part in |
| `--as netzbetreiber` | the network owner (S/PRI Leistungserbringer): S/PRI only, so the cross-interface rules are not checked |
| `--now <RFC 3339>` | the moment deadlines are judged at; default: the latest control timestamp of the files |

Each party sees only the messages it sends or receives; files of another switch, another
party's leg, symbolic links and sub-folders are listed as skipped. A folder holds at most
10 000 entries.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | everything read, no finding, nothing overdue |
| 1 | an error-level issue or finding, an overdue obligation, an unreadable file, or no switch to replay |
| 2 | a usage error |

A deadline that is only a window — the last moment for a Storno or a Terminverschiebung — does
not fail the run once it has passed. Terminal control characters in files and file names are
printed as `�`.
