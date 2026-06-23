# IMPRESS + IPSO readiness backlog

The build plan for the regulatory-readiness loop. **IMPRESS is the primary regulator;**
IPSO is a long-term/secondary target (a publisher can only be a *member* of one at a
time, so "ready for both" means our standards + processes satisfy **both** Codes).

**Loop rule:** build the top unbuilt item per iteration (agents where parallelisable)
→ open a **PR for human review** → tick it here. **Never auto-deploy regulatory
content** — the "regulated by" statement, policies and legal copy are the user's to
approve. Stop the loop when every item is done or blocked on the user.

Tags: **[code]** = build; **[content]** = editorial/legal copy the user must write/approve
(I draft a starting point); **S/M/L** = effort.

## Already in place (do NOT rebuild)
Per-article IMPRESS complaint flow (7-day/21-day timeline flags, emailed
acknowledgement + reference, IMPRESS escalation link, internal-notes + emailed-reply
thread); corrections with equal-prominence; hash-chained audit trail;
reporting-restriction + identification-risk flags on crawled leads; legal sign-off gate
before publish (active-proceedings firewall); standards / privacy / about / team /
contact pages; source-trust banner; self-hosted images.

## P1 — IMPRESS membership essentials (do first)
- [x] **1. "Regulated by IMPRESS" statement + mark in the footer** [code+content, S] — a
  regulated publisher must visibly state it's regulated by IMPRESS, with the IMPRESS
  mark + a link to IMPRESS and to our complaints process. Build: a footer block
  (src/components.rs / src/app.rs) + the statement text (user approves the exact
  wording + whether membership is live).
- [x] **2. Public Complaints policy page** (`/complaints`) [code+content, M] — a clear,
  standalone complaints procedure: how to complain (the per-article form already
  exists), our process + the IMPRESS 7-day acknowledge / 21-day decide timescales,
  and escalation to IMPRESS. Add a route + a footer link. (Per-article form already
  links to it conceptually; this is the policy/landing page.)
- [x] **3. Public Corrections & clarifications log** (`/corrections`) [code, M] — IMPRESS
  due-prominence: a public page listing published corrections (reads the existing
  corrections store) + a one-paragraph corrections policy. Footer link.

## P2 — transparency + governance (content-heavy; draft + user approves)
- [ ] **4. Ownership, funding & transparency statement** [content, S-M] — who owns/funds
  the publication; on /about or a /transparency page.
- [ ] **5. Whistleblowers' charter / source-protection policy** [content, S].
- [ ] **6. Conflicts-of-interest policy** [content, S].
- [ ] **7. Annual complaints / transparency report** [code+content, M] — surface complaint
  stats (counts, upheld rate, timeliness from the data we now record) + a published
  report. (The complaint timestamps already support the metrics.)

## P3 — editorial-gate enforcement (code; serves IMPRESS justice/children + IPSO 7/11)
- [ ] **8. Victim-anonymity / jigsaw-identification check in the publish gate** [code, M] —
  surface the `identification_risk` flag prominently at legal sign-off and require an
  explicit confirmation that no reporting restriction / victim-identification breach
  exists. Serves IMPRESS (children, justice) + IPSO Clause 7 (children in sex cases)
  and Clause 11 (victims of sexual assault — anti-jigsaw).
- [ ] **9. Spent-convictions / right-to-erasure (takedown) workflow + retention policy**
  [code+content, L] — a structured removal-request lane for the public conviction
  database (UK GDPR / Rehabilitation of Offenders). The complaints system is the
  intake; this adds the removal/retention handling.

## IPSO-specific (long-term; flag where it adds to IMPRESS)
- [ ] **10. Editors' Code adherence mapping** [content, S] — a doc mapping our practices to
  both the IMPRESS Standards Code and the IPSO Editors' Code (so switching regulators
  is a known quantity). Most clauses overlap; the IPSO-specific edges are Clause 7
  (children in sex cases) + Clause 11 (victims of sexual assault) — covered by item 8.

## Notes
- This is my own assessment (the audit agents died on a process restart); validate the
  exact IMPRESS scheme wording against impress.press before publishing the "regulated
  by" statement.
- Items 1–3 are the highest-value, mostly-code wins and the most visible membership
  requirements — **start there.**
