# Support runbook

## License issues (docs/13)
| Symptom | Action |
|---|---|
| `key_active_elsewhere` | User self-releases their own device in Settings → Account. Releasing ANOTHER device = admin `pnpm lic:admin release-device --key PRSM-… --instance <id>` (support-only). |
| `key_revoked` | Refund flow; the admin UI shows the revocation event trail. |
| Grace expired offline | User opens the app online once — auto-heal within one validate (docs/13 § 5.3). |
| Trial "reset" requests | One trial per install identity; support may issue a comp key via `pnpm lic:admin issue --sku yearly --note "comp: <ticket>"`. |

## Diagnostics bundle
Settings → Account → "Export diagnostics" (sku, expiry, last validation
result, instance id prefix ONLY — no license material, PRISM-LIC-030).

## Escalation
Engine defects: attach the diagnostics bundle + the scan target root +
`app.db` (user-consented). The arena canonical hash makes scan repro
byte-comparable across machines.
