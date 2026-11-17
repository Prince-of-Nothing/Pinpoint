# GitHub as the v1 host for per-country map packs

**Status:** Research note, v1 · **Date:** 2026-11-17 · **Scope:** where
`catalog.json` and country pack assets live; no code.
All limits checked against GitHub docs on 2026-11-17.

## 1. GitHub limits that matter

| Item | Value | Source |
|---|---|---|
| Git file warning | 50 MiB (push still succeeds) | [About large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github) |
| Git file hard block | 100 MiB; browser upload 25 MiB | same |
| Repo size | ideally <1 GB, <5 GB strongly recommended; newer page recommends ≤10 GB on-disk | [About large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github), [Repository limits](https://docs.github.com/en/repositories/creating-and-managing-repositories/repository-limits) |
| Release asset | <2 GiB per file, ≤1000 assets per release, **no** total-release or bandwidth limit | [About releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases) |
| Release retention | no documented expiry; assets live until the release/asset is deleted (deleting a release deletes its assets) | [Managing releases](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository) |
| Release serving | 302 to `release-assets.githubusercontent.com` (Azure Blob origin) behind Fastly; `Accept-Ranges: bytes`; signed URL expires ~1 h | observed with `curl -I`, 2026-11-17; host listed in [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) |
| `raw.githubusercontent.com` | repo files only, so ≤100 MiB; `Cache-Control: max-age=300`, Fastly, `Accept-Ranges: bytes`; per-IP unauthenticated limits (tightened May 2025, exact number unpublished) | [Repository limits](https://docs.github.com/en/repositories/creating-and-managing-repositories/repository-limits), [changelog](https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/), observed headers |
| Actions, public repos | standard runners free and unlimited; job ≤6 h, run ≤35 days | [Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [Actions limits](https://docs.github.com/en/actions/reference/limits) |
| `ubuntu-latest` (public) | 4 vCPU, 16 GB RAM, 14 GB SSD | [Hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) |
| Actions artifacts | 90-day default retention, 500 per job, download requires login → not a CDN | [Artifacts](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts), [upload-artifact](https://github.com/actions/upload-artifact#limitations) |
| Git LFS | Free: 10 GiB storage + 10 GiB/mo bandwidth; overage $0.07/GiB-month, $0.0875/GiB downloaded | [LFS billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs), [2024 changelog](https://github.blog/changelog/2024-06-03-new-enterprise-accounts-have-metered-billing-for-git-lfs/) |

**Policy.** Acceptable Use §9: if bandwidth is "significantly excessive in
relation to other users of similar features", GitHub may "suspend your Account,
throttle your file hosting, or otherwise limit your activity" and may delete
repos placing "undue strain" on its infrastructure; §6 forbids reselling the
Service. Releases are the documented way to distribute large binaries, but
there is no SLA and no contractual egress commitment.
([AUP](https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies),
[About large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github))

## 2. 200 GB of packs, ~1 TB/month egress

| | GitHub Releases | Cloudflare R2 (Standard) | Backblaze B2 |
|---|---|---|---|
| Storage | $0 (unmetered) | $0.015/GB-mo → 190 GB billable ≈ **$2.85/mo** | $6.95/TB-mo → 190 GB ≈ **$1.32/mo** |
| Egress | $0, subject to AUP §9 | **$0** | free to 3× stored (600 GB); 1 TB → 400 GB × $0.01 = **$4.00** |
| Ops | free | 10M Class B/mo free, then $0.36/M | Class A–C free |
| Range | yes (`Accept-Ranges: bytes`) | yes (`GetObject` Range) | yes, **single range only**, 206 |
| Total @1 TB/mo | **$0** | **≈$2.85/mo** | **≈$5.32/mo** (≈$1.32 below 600 GB) |

Prices: [R2 pricing](https://developers.cloudflare.com/r2/pricing/) (updated
2026-10-30), [B2 pricing](https://www.backblaze.com/cloud-storage/pricing)
($6.95/TB, free 3× egress). Range: [R2 S3 API](https://developers.cloudflare.com/r2/api/s3/api/),
[B2 download](https://www.backblaze.com/apidocs/b2-download-file-by-name).

## 3. Recommendation

**v1: GitHub Releases, one asset per country, hard cap <2 GiB, target
≤1.5 GiB.** 200 GB total and 1 TB/month of downloads is well inside GitHub's
documented limits and costs nothing; 2 GiB × 250 countries is far below the
1000-asset ceiling.

Rules:
- CI hard-fails any asset ≥2 GiB and any pack set >100 GB.
- If a country exceeds 1.5 GiB, split by zoom band (`z0-9`, `z10-12`,
  `z13-14`) or into ~1 GiB `.partN` files in the same release; the catalog
  pins every name, size and SHA-256.
- Do **not** put packs in git, LFS, raw URLs or Actions artifacts: 100 MiB
  file cap, metered bandwidth, 90-day expiry, or login-walled downloads.
- Move to R2 when total packs >100 GB, monthly egress >1–2 TB, or you need an
  SLA/analytics/immutable URLs. At 200 GB R2 is ~$3/month with free egress;
  B2 is cheaper only while egress stays under 3× storage.

**Recommended v1 limit: 1.5 GiB target / <2 GiB hard per country asset.**
