# PH Press — Regulatory Code Adherence Mapping

**Publication:** Predator Hunters Press (predatorhunters.co.uk)  
**Category:** UK court-reporting / predator-exposure newsroom  
**Primary regulator:** IMPRESS — *intended* (we hold ourselves to the IMPRESS Standards Code and intend to seek registration; not yet a member). The "Regulated by IMPRESS" footer statement is runtime-gated on the `regulator_registered` setting and defaults **off** — the site makes no regulated-by claim until registration is confirmed and an admin flips the flag.  
**Long-term consideration:** IPSO (Editors' Code) — for visibility to mainstream advertisers and as a switching baseline  
**Document purpose:** Demonstrate readiness for IMPRESS registration; quantify the work required to switch to (or dual-map against) IPSO; feed the compliance backlog.  
**Last reviewed:** 2026-06-23

> **Provenance note:** IMPRESS clause wording confirmed against impressorg.com and presscouncils.eu mirrors (impress.press returned ECONNREFUSED at time of drafting; PDF binary only). IPSO clause wording confirmed against editorscode.org.uk and societyofeditors.org mirrors (ipso.co.uk returned HTTP 403). The 2025 update to IPSO Cl.6 (Children) has been noted. Both codes should be re-verified against the official PDFs before formal registration — the substance captured here is accurate for mapping purposes.

---

## Compliance Surface Reference

The platform features referenced throughout this document are:

| Reference | What it is |
|---|---|
| **Footer statement** | "Regulated by IMPRESS" + link to IMPRESS — runtime-gated on `regulator_registered` (default **off**; shown only once registered). The /complaints and /corrections links always render. |
| **`/complaints`** | Public complaints policy: 7-day acknowledge / 21-day final response; escalation to IMPRESS |
| **Complaints workflow** | Per-article "Make a complaint" form; 8 IMPRESS statuses; emailed acknowledgement + reference number; internal notes; emailed replies; overdue flags |
| **`/corrections`** | Public corrections & clarifications log; equal prominence; both versions retained on record |
| **`/governance`** | Ownership & funding transparency (sole trader, self-funded); conflicts-of-interest policy; whistleblowers' charter & source-protection policy |
| **`/complaints-report`** | Public, aggregate-only complaints transparency report (no PII) |
| **`/standards`** | Editorial standards page |
| **`/privacy`** | Privacy policy |
| **Publish legal sign-off gate** | Server-enforced: story can reach Published only from Legal-Review/Scheduled; 4-point pre-publish checklist including explicit victim-anonymity / jigsaw-identification confirmation; sign-off recorded in hash-chained audit trail (actor + timestamp) |
| **Crawler flags** | Reporting-restriction + identification-risk flags raised at article intake |
| **AI labelling** | AI-assisted drafts are labelled |
| **No-name-before-charge** | Editorial practice: no individual named before charge without documented justification |
| **Self-hosted images** | Images hosted on own infrastructure (no third-party CDN leakage of identities) |

---

## Section A — IMPRESS Standards Code (Primary Regulator)

Standards Code issued February 2023. Ten standards; Public Interest is a cross-cutting exception framework, not a numbered standard.

**Column key — Enforcement type:**
- **Platform** = enforced by server logic; cannot be bypassed without a code change
- **Editorial** = policy/practice; depends on journalist compliance
- **Hybrid** = platform constraint + editorial obligation
- **N/A** = clause category does not apply to this publication's activities

| # | Standard | How we comply | Enforcement | Gap / TODO |
|---|---|---|---|---|
| 1 | **Accuracy** | `/corrections` log maintains a public record of corrections with equal prominence; both original and corrected versions retained. Corrections workflow within the complaints system triggers a correction article when appropriate. `/standards` states the accuracy commitment. AI-assisted drafts are labelled, preventing AI output being presented as verified fact. | Hybrid | No automated fact-verification tooling. The distinction between fact, conjecture, and opinion relies on editorial discipline; no platform label or template enforces it. TODO: add a structured "Opinion / Analysis" content-type flag at publish time. |
| 2 | **Attribution & Plagiarism** | Editorial practice requires source citation in copy. AI-labelling requirement indirectly covers AI-generated attribution. `/corrections` handles prompt correction of attribution failures. | Editorial | No automated plagiarism or uncredited-re-use detection. TODO: document attribution policy in the `/standards` page explicitly. |
| 3 | **Children** | Crawler identification-risk flag raised at intake on any story involving minors. Publish legal sign-off gate includes explicit jigsaw-identification confirmation covering child subjects. No-name-before-charge practice applies. IMPRESS Cl.3 permits general scene images without individual identification — consistent with our self-hosted image policy. | Hybrid | No formal written policy for consent from responsible adults when interviewing under-16 sources (distinct from identifying children in proceedings). TODO: add a "child-subject interview" protocol to the editorial standards page. |
| 4 | **Discrimination** | `/standards` states the non-discrimination commitment. Editorial practice prohibits prejudicial references to protected characteristics; the hate-incitement bar is editorial. | Editorial | No platform-level content flag for discriminatory language. Moderation is entirely editorial. TODO: consider a self-assessment checklist item in the pre-publish gate for discrimination risk. |
| 5 | **Harassment** | Editorial practice: journalists must identify themselves and cease contact when requested. Source-protection policy on `/governance` covers whistleblowers specifically. No policy on deceptive newsgathering is needed for standard reporting; subterfuge would require documented public-interest justification. | Editorial | No written documented procedure for logging contact-cessation requests or journalist identification steps. TODO: add a brief harassment/contact protocol to `/standards`. |
| 6 | **Justice** | **Platform-enforced firewall:** a story can reach Published only via Legal-Review or Scheduled states — the system prevents bypassing legal scrutiny. **Pre-publish checklist (4 points):** includes explicit victim-anonymity confirmation and jigsaw-identification check. **Audit trail:** every sign-off is hash-chained with actor + timestamp. **Crawler reporting-restriction flags** surface any active-proceedings / reporting-restriction risk at intake. **No-name-before-charge** is an established editorial practice. Sexual-offence victim anonymity is confirmed in the pre-publish checklist (aligns with Cl.6.4). No payments to witnesses or defendants are made (Cl.6.5) — sole-trader, self-funded operation. | Platform | Sub-clause 6.3 (no identification before charge) is editorial practice only; the platform flags the risk but cannot stop publication of a named-before-charge piece if an editor overrides. TODO: add a hard block requiring documented public-interest override for name-before-charge. |
| 7 | **Privacy** | `/privacy` policy published. Publish checklist includes a privacy-intrusion consideration step. Self-hosted images prevent identity leakage through third-party CDN. Covert information gathering would require a documented public-interest case. Cl.7.3 (anonymisation requests) — `/corrections` provides the correction mechanism; no formal anonymisation-request intake form yet. | Hybrid | No formal anonymisation / right-to-erasure request intake process beyond general corrections. TODO: add an anonymisation-request route to `/complaints` or a dedicated form, and document handling in editorial policy. |
| 8 | **Sources** | `/governance` publishes a whistleblowers' charter and source-protection policy. Fabricated sources are prohibited (editorial). Payments to public officials are not made (sole trader, no budget for paid tipsters). | Hybrid | Source-protection policy exists on `/governance` but is not cross-referenced from `/standards`. TODO: link source-protection policy from the editorial standards page. The policy should explicitly reference Cl.8.1 confidentiality and the "manifestly dishonest" exception. |
| 9 | **Self-Harm & Suicide** | Editorial practice avoids excessive method details. | Editorial | **Gap:** The IMPRESS Code requires publishers to "signpost and link to relevant support services and resources" (e.g. Samaritans, PAPYRUS) in suicide/self-harm stories. No platform template, no editorial checklist item, and no written policy covers this requirement. TODO: (a) add a Samaritans/support-link signpost requirement to the pre-publish checklist; (b) create a reusable "support resources" content block that editors can insert; (c) document in `/standards`. |
| 10 | **Transparency** | `/governance` discloses ownership (sole trader), funding (self-funded, no external funding), and conflicts-of-interest policy. `/complaints-report` is a public, aggregate-only transparency report. Footer "Regulated by IMPRESS" statement is runtime-gated on `regulator_registered` (default off; shown only once registered). AI-assisted content is labelled (Cl.10.5 — AI oversight and labelling). Paid editorial content: not applicable (no sponsored content accepted). Financial-product coverage: not applicable. | Hybrid | Cl.10.3 (financial product interest disclosure) is N/A currently; if financial/advertorial content is ever accepted, this policy must be documented first. AI labelling relies on editorial discipline — no platform-level AI content-type flag yet. TODO: add an AI-content flag to the publish workflow. |

### IMPRESS Public Interest Framework

The IMPRESS Public Interest exception is a cross-cutting defence that applies to Standards 3, 5, 6, 7, 8, and 9. Our current compliance position: public-interest overrides are not systematically documented before the fact. The Justice firewall and pre-publish checklist capture the outcome, but do not yet record the pre-action public-interest reasoning log that IMPRESS expects editors to create. **TODO: add a structured public-interest justification field to the Legal-Review workflow for any story flagging a potential code tension.**

---

## Section B — IPSO Editors' Code of Practice

Clauses 1–16. Asterisked clauses (*) permit Public Interest exceptions (note: Cl.1 Accuracy carries no Public Interest defence); the Public Interest framework is described after the table.

Note: IPSO and IMPRESS share substantial overlap. Where features map to both, this is noted.

| # | Clause | How we comply | Enforcement | Gap / TODO |
|---|---|---|---|---|
| 1 | **Accuracy** | `/corrections` log with equal-prominence corrections and both versions retained. Complaints workflow triggers corrections. AI-draft labelling prevents unverified AI output being presented as fact. `/standards` states accuracy commitment. Distinction between fact and opinion: editorial practice only. | Hybrid | Same gap as IMPRESS Cl.1: no structured "Opinion / Analysis" content-type flag. Defamation outcome reporting (Cl.1(iv)) is editorial. Accuracy carries no Public Interest defence — inaccuracy can never be justified on PI grounds. |
| 2 | **Privacy*** | `/privacy` policy published. Self-hosted images. Pre-publish checklist includes privacy consideration. Covert gathering requires public-interest justification. Right to private life in digital communications: editorial awareness only, not a platform control. | Hybrid | No formal anonymisation-request intake process (same gap as IMPRESS Cl.7). |
| 3 | **Harassment*** | Editorial practice: journalists identify themselves and cease contact on request. Source-protection policy on `/governance`. | Editorial | No logged contact-cessation procedure. Same gap as IMPRESS Cl.5. |
| 4 | **Intrusion into Grief or Shock** | Editorial practice: approach with sympathy and discretion. | Editorial | **Gap (editorial-only):** No written protocol or checklist item for grief/shock situations. This is inherently an editorial judgement; the platform cannot enforce it. TODO: add a brief grief-and-shock guidance note to `/standards`. |
| 5 | **Reporting Suicide*** | Editorial practice avoids excessive method detail. | Editorial | **Gap:** No Samaritans/support-service signpost requirement and no checklist prompt. Same gap as IMPRESS Cl.9, and more specifically: IPSO Cl.5 requires avoiding "excessive detail of the method used." IMPRESS Cl.9 additionally requires signposting support resources. Both are unmet at the platform level. TODO: same as IMPRESS Cl.9. |
| 6 | **Children*** | Crawler identification-risk flags. Pre-publish jigsaw-identification confirmation. Under-18 anonymity in proceedings covered by the Justice firewall + checklist. No school photography without authority permission: editorial. Payment to under-16s for welfare material: N/A (we do not pay sources). | Hybrid | No formal written adult-consent protocol for interviewing under-16 sources (same gap as IMPRESS Cl.3). **Note:** IPSO updated Cl.6 (effective 2025); the 2025 wording should be verified against the official PDF before formal registration. |
| 7 | **Children in Sex Cases*** | Pre-publish checklist includes explicit victim-anonymity / jigsaw-identification confirmation. Platform firewall prevents publication without Legal-Review. Self-hosted images prevent identification via CDN. "Incest" terminology avoidance: editorial. Implied-relationship avoidance: editorial. | Hybrid | Partly editorial: the platform confirms the checklist is completed, but the editor makes the identification judgement. No automated jigsaw-identification detection. |
| 8 | **Hospitals*** | Editorial practice: journalists identify themselves to hospital staff. Permission required before entering non-public areas. | Editorial | **Gap (editorial-only, likely N/A in practice):** PH Press is a court-reporting publication; hospital-floor newsgathering is not a routine activity. No written hospital access protocol. TODO: document briefly in `/standards` for completeness. |
| 9 | **Reporting of Crime*** | Pre-publish checklist and Justice platform firewall cover: no name before charge; victim anonymity; under-18 protections in proceedings. Crawler flags surface reporting restrictions. No-name-before-charge is an established practice. Child-witness/victim identification: covered by Cl.6/7 controls above. Relatives/friends not identified without consent unless relevant: editorial. | Hybrid | Sub-clause on relatives/friends (Cl.9*i) is editorial-only. Under-18 arrest naming (Cl.9*iii): editorial practice backed by crawler flag, but no platform hard-block before youth court appearance. |
| 10 | **Clandestine Devices & Subterfuge*** | Editorial practice: no hidden cameras or listening devices. Communications interception prohibited. Misrepresentation requires public-interest justification and must be the only available means: editorial policy. | Editorial | No platform-enforced public-interest override log for subterfuge decisions. Same gap as IMPRESS Public Interest framework note. |
| 11 | **Victims of Sexual Assault*** | Pre-publish checklist includes explicit victim-anonymity confirmation. Platform firewall requires Legal-Review. "Journalists may enquire but must exercise discretion" — editorial. | Hybrid | Same as Cl.7: identification judgement is editorial; no automated identification detection. |
| 12 | **Discrimination** | `/standards` states commitment. Editorial practice avoids prejudicial references. | Editorial | Same gap as IMPRESS Cl.4: no platform-level discrimination-language flag. |
| 13 | **Financial Journalism** | Sole trader, self-funded: no financial-product coverage, no securities trading relevant to editorial output. Conflicts disclosed on `/governance`. | N/A | N/A in current operation. If financial coverage is ever commissioned, a written conflict-of-interest and trading-restriction policy must be documented. |
| 14 | **Confidential Sources** | `/governance` publishes the whistleblowers' charter and source-protection policy. Covers IPSO's "moral obligation to protect confidential sources." | Hybrid | Source-protection policy not cross-linked from `/standards` (same gap as IMPRESS Cl.8). |
| 15 | **Witness Payments in Criminal Trials*** | Sole trader, self-funded: no payment to witnesses. IMPRESS Justice Cl.6.5 also prohibits payments to witnesses or defendants. | Hybrid | **Gap (policy documentation):** No explicit written policy stating that we do not pay witnesses, and setting out what we would do if circumstances changed. IMPRESS Cl.6.5 provides the legal prohibition; IPSO Cl.15 adds the procedural requirements (disclosure to prosecution/defence, no conditional payment). TODO: add a brief "Payments policy" section to `/governance` stating the no-payment position and IPSO Cl.15 disclosure obligations. |
| 16 | **Payment to Criminals*** | Sole trader, self-funded: no payments to convicted or confessed criminals or their associates for story material. | Hybrid | **Gap (policy documentation):** Same as Cl.15 — no explicit written policy. TODO: include in the Payments policy on `/governance`. |

### IPSO Public Interest Framework

The Public Interest exception applies to clauses marked (*). Where editors invoke it, they must demonstrate a reasonable belief that publication is proportionate to the public interest served. For children under 16, an "exceptional" public interest must be demonstrated.

Our position: for Justice/anonymity-related decisions, the platform firewall forces Legal-Review, which is the closest thing to documented public-interest reasoning. However, there is no structured public-interest justification log distinct from the Legal-Review sign-off. **TODO: same as IMPRESS Public Interest note — add a public-interest justification field to the Legal-Review workflow.**

---

## Cross-Code Mapping Summary

The following features serve requirements in **both** codes simultaneously:

| Platform feature | IMPRESS standards | IPSO clauses |
|---|---|---|
| Pre-publish checklist (victim anonymity / jigsaw ID) | Cl.6 (Justice) | Cl.7, Cl.11 |
| Platform firewall (Legal-Review gate) | Cl.6 (Justice) | Cl.1, Cl.7, Cl.9, Cl.11 |
| `/corrections` (equal prominence, both versions) | Cl.1 (Accuracy) | Cl.1 |
| Crawler reporting-restriction + ID-risk flags | Cl.6 (Justice) | Cl.6, Cl.7, Cl.9 |
| No-name-before-charge practice | Cl.6 (Justice) | Cl.9 |
| `/governance` source-protection policy | Cl.8 (Sources) | Cl.14 |
| Self-hosted images | Cl.7 (Privacy) | Cl.2 |
| AI-draft labelling | Cl.10 (Transparency) | Cl.1 (accuracy of information) |
| `/complaints` + complaints workflow | Cl.1, Cl.6, Cl.7 | Cl.1–Cl.3 |

---

## Outstanding Before Formal IMPRESS Registration

The following items must be resolved before formal registration or regulatory sign-off. Items are roughly priority-ordered.

1. **Suicide/self-harm signpost [IMPRESS Cl.9 / IPSO Cl.5] — HIGH**  
   Add a Samaritans/support-link signpost requirement to the pre-publish checklist; create a reusable "support resources" content block for editors; document in `/standards`.

2. **Public-interest justification log in Legal-Review workflow [IMPRESS Public Interest / IPSO Public Interest] — HIGH**  
   The platform forces Legal-Review but does not record the pre-action public-interest reasoning IMPRESS expects. Add a structured justification field (reason for override, proportionality assessment) to the Legal-Review workflow.

3. **Hard-block for name-before-charge override [IMPRESS Cl.6.3 / IPSO Cl.9] — HIGH**  
   Currently a flag, not a block. Add a mandatory documented-justification step before a named-before-charge story can be approved.

4. **Anonymisation/erasure request intake [IMPRESS Cl.7.3 / IPSO Cl.2] — MEDIUM**  
   Add a formal anonymisation-request route (form or dedicated email) and document the handling procedure. Link from `/corrections` and `/privacy`.

5. **Payments policy on `/governance` [IPSO Cl.15, Cl.16 / IMPRESS Cl.6.5] — MEDIUM**  
   Add a brief written "Payments to sources, witnesses, and criminals" policy section to `/governance` stating the no-payment position and the IPSO Cl.15 disclosure obligations if circumstances change.

6. **Child-subject interview consent protocol [IMPRESS Cl.3 / IPSO Cl.6] — MEDIUM**  
   Document the procedure for obtaining responsible adult consent before interviewing under-16 sources (distinct from anonymity in proceedings). Add to `/standards`.

7. **Source-protection policy cross-link [IMPRESS Cl.8 / IPSO Cl.14] — LOW**  
   Cross-link the `/governance` source-protection policy from `/standards`. Policy content exists; discoverability gap only.

8. **"Opinion / Analysis" content-type flag [IMPRESS Cl.1 / IPSO Cl.1] — LOW**  
   Add a structured content-type label at publish time to distinguish editorial opinion/analysis from news fact.

9. **Contact-cessation logging procedure [IMPRESS Cl.5 / IPSO Cl.3] — LOW**  
   Add a brief harassment/contact protocol to `/standards`, including how journalists log contact-cessation requests.

10. **Grief/shock guidance note [IPSO Cl.4] — LOW (editorial-only)**  
    Add brief grief-and-shock sensitivity guidance to `/standards`. No platform enforcement possible; editorial practice only.

11. **Hospital access protocol [IPSO Cl.8] — LOW (likely N/A)**  
    Document briefly in `/standards` for completeness even though hospital-floor newsgathering is not a routine activity.

12. **Re-verify clause wording against official PDFs — BEFORE REGISTRATION**  
    impress.press and ipso.co.uk were inaccessible at time of drafting. Verify all clause wording and numbering against the official PDFs before submitting to IMPRESS. Pay particular attention to IPSO Cl.6 (2025 update to Children clause).

---

*This document is an internal compliance mapping for editorial and development use. It is not a public-facing regulatory submission.*
