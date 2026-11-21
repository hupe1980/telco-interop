+++
title = "How a provider switch works in Germany"
description = "Background for newcomers: § 59 TKG, the AK S&P, WBCI, S/PRI and WITA, the three parties of a switch and the messages they exchange."
weight = 1

[extra]
nav = "Introduction"
seo_title = "How the German provider switch works: § 59 TKG, WBCI, S/PRI · telco-interop"
+++

When a customer in Germany moves a fixed-line contract to another provider, the providers
have to coordinate: the old contract ends on the day the new one starts, the line is handed
over or switched, and the phone numbers move with the customer. The law sets the frame, an
industry working group writes the interfaces, and every provider implements them.

## The law: § 59 TKG

§ 59 of the Telekommunikationsgesetz (TKG) governs the *Anbieterwechsel* (provider switch,
also *Providerwechsel*) and the *Rufnummernportierung* (number porting). The losing provider
keeps the service running until the switch is done, and the service may be „nicht länger als
einen Arbeitstag unterbrochen" (interrupted for no longer than one working day). So the switch
runs on agreed dates and working-day deadlines that both sides can check.

## The interfaces: AK S&P

The *Arbeitskreis Schnittstellen & Prozesse* (AK S&P), hosted by the industry associations VATM
and BUGLAS, specifies the interfaces the providers use. Three of them matter for a switch:

| Interface | Between | Used for | Crate |
|---|---|---|---|
| **WBCI** | provider ↔ provider | agreeing the switch: date, line takeover, numbers to port, cancellation, postponement | [`wbci`](@/docs/wbci/_index.md) |
| **S/PRI** | service provider ↔ network owner | ordering, changing, cancelling and repairing lines on open-access (mostly fibre) networks, including the provider-switch order | [`spri`](@/docs/spri/_index.md) |
| **WITA** | service provider ↔ Deutsche Telekom | the same for lines on Telekom's network (Telekom's own interface) | `wita-kit`, name reserved |

All three are XML over SOAP web services. WBCI's name, „WITA Based Carrier Interface", shows
its descent from WITA; the message names (ABBM, ERLM, …) are shared across the family.

## The three parties

| Party | WBCI name | S/PRI name | Does |
|---|---|---|---|
| Gaining provider | **EKPauf** (aufnehmender Endkundenvertragspartner) | Auftraggeber | wins the customer, asks for the switch, orders the line |
| Losing provider | **EKPabg** (abgebender Endkundenvertragspartner) | abgebender Provider | checks the request against its contract, answers with a date or a refusal, hands over the line and the numbers |
| Network owner | — | **Leistungserbringer** (NGA, NGAB) | owns the physical line, executes the order on the switch date |

The network owner can be the losing provider itself (it owns the line: WBCI alone is enough),
Deutsche Telekom (the order goes over WITA) or an open-access fibre network that sells to
many providers (the order goes over S/PRI).

## One switch, step by step

1. **Pre-agreement (WBCI).** The gaining provider sends a *Vorabstimmung*: `VA-KUE-MRN` (switch
   with number porting), `VA-KUE-ORN` (without) or `VA-RRNP` (porting only). Every call is
   acknowledged at once with a technical receipt (**TEQ**).
2. **The losing provider's answer (WBCI).** Within 3 working days it compares the request with
   its contract record and answers **RUEM-VA** (agreed, with the binding switch date) or
   **ABBM** (refused, with codes such as 8103 „Rufnummer nicht geschaltet").
3. **Line takeover (WBCI).** The gaining provider announces with **AKM-TR** whether it takes
   over the existing line (by its LineID). Until the switch date either side may cancel
   (`STR-AUF`, `STR-AEN`) and the gaining provider may postpone (`TVS-VA`); those are answered
   with **ERLM** or **ABBM**.
4. **The order (S/PRI).** On an open-access network the gaining provider orders the line with a
   provider-switch order (**PV**) that carries the WBCI Vorabstimmungs-ID. The network owner
   confirms receipt (**QEB**), tells the losing provider (**AKM-PV**, answered with
   **RUEM-PV**), confirms the date to both (**ABM**, **ABM-PV**), reports completion on the
   switch date (**ERLM**, **ERLM-PV**) and the billing date (**ENTM**, **ENTM-PV**).
5. **Number porting.** On the switch date the numbers move in the agreed porting window
   (`ZF2`, 6 to 12 o'clock, by default).

The [one-switch guide](@/docs/across-both/one-switch.md) follows exactly this sequence in code.

## Where the crates fit

The crates are the protocol layer of such a system, written as plain values and functions
without I/O (*sans-I/O*):

- **Your application** receives a SOAP call, verifies its signature, stores it, and answers.
- **The crates** turn the payload into typed values and back, predict whether the peer will
  refuse what you send, compute every deadline with its explanation, keep the state of each
  pre-agreement and order, and decide the losing provider's answer.

Transport, WS-Security, TLS, persistence and the clock stay in your application, so the same
code runs in a gateway, a batch job, a test or a support tool. [Scope and interpretations](@/docs/reference/scope.md)
lists exactly what is implemented.

Next: [build the first pre-agreement](@/docs/start/getting-started.md), or look up a term in the
[glossary](@/docs/start/glossary.md).
