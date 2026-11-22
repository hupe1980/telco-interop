+++
title = "telco-interop"
description = "Rust crates for the German provider switch (Anbieterwechsel, § 59 TKG): WBCI 2.0 and S/PRI 4.x, typed, sans-I/O, proven against the official XSDs."
template = "index.html"

[extra]
seo_title = "WBCI & S/PRI in Rust: the German provider switch · telco-interop"

[[extra.features]]
title = "Typed, all the way down"
text = "A <code>VorabstimmungsId</code> exists only if it is valid. Requests mirror the schema's <code>xsi:type</code>, mandatory elements are plain fields, so an incomplete request does not compile."
[[extra.features]]
title = "Know before you send"
text = "28 cited WBCI rules and 34 S/PRI rules predict the peer's refusal: wrong code for the case, a lead time too short, a Terminverschiebung where the business case has none. Each issue names the element, the clause and the code the peer would answer with."
[[extra.features]]
title = "Deadlines that explain themselves"
text = "„3 AT after the TEQ“, „before 0:00 of X−3 AT“, „10 Werktage after the TAM“: computed in German time, with every counted and skipped day and why."
[[extra.features]]
title = "The whole process as a value"
text = "A sans-I/O state machine per WBCI pre-agreement and per S/PRI order refuses what the protocol forbids, with the code to answer, and tells each side what it may do next."
[[extra.features]]
title = "The losing provider's answer, decided"
text = "Hand over your contract record and get the RUEM-VA or ABBM the check table of Anlage 1 §2.4 prescribes: names compared by sound with the AK S&amp;P's word rules, explained field by field."
[[extra.features]]
title = "One switch, both interfaces"
text = "<code>Wechsel</code> merges the WBCI pre-agreement and its S/PRI orders into one timeline with both interfaces' deadlines, and checks six rules that the two agree."
[[extra.features]]
title = "Safe with hostile input"
text = "No DTDs, no entity expansion; size, depth, text and every repetition bounded; fuzzed; never a panic. Personal data never appears in <code>Debug</code>, errors or issues."
[[extra.features]]
title = "A command line for the files"
text = "<code>brew install hupe1980/tap/telco-interop</code>: read and check message files, explain a code, replay a switch from one party's view with findings and deadlines. No personal data in its output."
[[extra.features]]
title = "No transport, no lock-in"
text = "The crates read and write the payloads, bare or in a SOAP 1.1 envelope. HTTPS, WS-Security, storage and your clock stay in your application."

[[extra.crates]]
name = "wbci"
interface = "WBCI 2.0"
between = "provider ↔ provider: the § 59 TKG switch, number porting, the losing provider's check"
status = "complete"
ok = true
[[extra.crates]]
name = "spri"
interface = "S/PRI 4.x"
between = "service provider ↔ network owner: open-access orders, faults, diagnostics, the provider switch"
status = "complete"
ok = true
[[extra.crates]]
name = "telco-interop"
interface = "both"
between = "one dependency for both, plus Wechsel: one switch across WBCI and S/PRI, and the command line"
status = "complete"
ok = true
[[extra.crates]]
name = "wita-kit"
interface = "WITA"
between = "service provider ↔ Deutsche Telekom: wholesale orders on Telekom lines"
status = "name reserved"
ok = false

[[extra.faq]]
q = "Is this official or certified?"
a = "No. telco-interop is an independent open-source project, not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom. Every payload kind the writers produce is validated against the official XML schemas on every CI run (373 WBCI and 776 S/PRI payloads), but certification of your system with your peers remains your step."
[[extra.faq]]
q = "Which specification versions are implemented?"
a = "WBCI 2.0 with the XSD V2.00, Anlage 1 „Spezifikation Vorabstimmung“ V5.4.1 and the Arbeitshandbuch WBCI 2.10; S/PRI 4.x with the schema 4.00.03, the Providervereinbarung S/PRI V4.4, Anlage 7 (EPKs) V.13 and the Arbeitshandbuch S/PRI 4.x V2.9.5. The scope page lists every document and each interpretation."
[[extra.faq]]
q = "Does it do SOAP, WS-Security or HTTPS?"
a = "It reads and writes the payloads bare or inside a SOAP 1.1 envelope, and leaves the SOAP header empty for your WS-Security signer. The crates are sans-I/O: transport, TLS, signing and verification, persistence and the clock belong to your application, so they fit any HTTP stack, queue or test harness."
[[extra.faq]]
q = "How are the deadlines counted?"
a = "In German time (Europe/Berlin, compiled in). WBCI working days (AT) are Monday to Friday without the nine nationwide public holidays; S/PRI Werktage are Monday to Saturday. Forwards deadlines end at 23:59 of the n-th working day, backwards ones before 0:00. Every deadline names its rule and source and lists the days it counted and skipped; other calendars, regional holidays and closing days are configurable."
[[extra.faq]]
q = "Can I use only WBCI, or only S/PRI?"
a = "Yes. Depend on the wbci or the spri crate directly, or on telco-interop with one feature turned off. The identifiers both interfaces share (Vorabstimmungs-ID, LineID, carrier code) are the same types in both crates, so adding the other one later needs no conversion."
[[extra.faq]]
q = "What about personal data?"
a = "Names, addresses, phone numbers, e-mail addresses, free text and attachment content print redacted in Debug output, and errors, validation issues, timelines and findings never contain them. The losing provider's check explains its decision without personal data, so the explanation can be logged and shared."
[[extra.faq]]
q = "Is WITA supported?"
a = "No. The wita-kit crate is a name reservation for Deutsche Telekom's wholesale ordering interface; nothing is implemented."
[[extra.faq]]
q = "What is the licence?"
a = "MIT or Apache-2.0, at your option. The AK S&P's specifications are copyrighted documents and are not part of the repository or the packages: the crates carry their own types and synthetic test data, and the schemas are fetched by URL and pinned by SHA-256 for the conformance test."
+++

{% <rust> %}
```rust
use wbci::pruefung::{self, Bestand};
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-mrn.xml"))?;
# let received: Timestamp = "2026-11-23T08:30:00Z".parse()?;
// A pre-agreement (VA-KUE-MRN) arrives from the gaining provider.
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
println!("{}", process.deadlines()[0]); // EKPabg: by Do 26.11.2026 23:59 …
# assert!(process.deadlines()[0].to_string().starts_with("EKPabg: by Do 26.11.2026 23:59"));
# Ok(())
# }
```
{% </rust> %}
