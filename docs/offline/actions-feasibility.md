# GitHub Actions feasibility — offline packs

**Assessed 2026-11-17.** `scripts/offline/build.ps1` + Planetiler, Valhalla,
`tools/native-router` and the graph gzip step on hosted `ubuntu-latest`.

| Region | PBF (2026-11-17) | Peak disk (est.) | Job time (est.) | Verdict |
|---|---:|---:|---:|---|
| Moldova | 101.5 MB | 3–4 GB | 15–25 min | **GO** |
| Romania | 333.7 MB | 6–8 GB | 30–45 min | **GO** |
| Ukraine | 884.9 MB | 10–15 GB | 60–100 min | **NO-GO on stock runner**; GO with ≥40 GB free |

`ubuntu-latest` is Ubuntu 24.04 with 4 vCPU, 16 GB RAM, a **14 GB SSD** and a
**6 h** job cap; standard runners are **free and unlimited on public repos**
([reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[limits](https://docs.github.com/en/actions/reference/limits)). The image has
Docker 28.0.4, **pwsh 7.6.10**, Rust 1.99.1, .NET SDK 8–11, gcc, `tar` and
`curl` ([image](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md)).
Avoid `ubuntu-slim` (1 vCPU, 5 GB, 15 min, no Docker-in-Docker). Public image
pulls are exempt from Docker Hub limits on hosted runners; self-hosted is
100 anonymous / 200 personal per 6 h
([GitHub](https://docs.github.com/en/actions/reference/limits#docker-hubs-rate-limit-for-github-actions),
[Docker](https://docs.docker.com/docker-hub/usage/pulls/)).

## Can the tools run?

- **Planetiler:** yes. 0.5× the PBF in RAM; **1 GB + 5–10× the PBF** in free
  disk, so Ukraine's working set is 5.4–9.8 GB
  ([requirements](https://github.com/onthegomap/planetiler#requirements)).
- **Valhalla:** yes. The script fixes `--mjolnir-concurrency 2`; Moldova's
  measured build was **114 s** ([moldova.md](moldova.md)). 16 GB is ample.
- **Rust / native-router:** yes. Rust and gcc are preinstalled; Moldova's graph
  is 110.9 MB at 283 MiB peak, 7–20 s ([native-routing.md](native-routing.md)).
  Ukraine is ~9× the input: ~1 GB graph, ~3 GB peak, minutes.
- **.NET gzip:** yes (SDK present), but no .NET project exists here. Use the
  `gzip` CLI; `GZipStream` is single-threaded and adds a csproj for nothing.

## PowerShell 7 on Linux

Portable by static review: `Get-Command -CommandType Application`, `Join-Path` /
`Resolve-Path` / `[IO.Path]::GetFullPath`, the `.exe` fallbacks, GNU
`tar -C … -cf …` / `-tf`, `docker -v "${workDir}:/data"` (a valid absolute Linux
mount), `Get-FileHash`, `Set-Content -Encoding ASCII` and the JSON cmdlets all
work. Gotchas: invoke **`pwsh -File`**, not `powershell`; `curl.exe` is absent
but `download.ps1` falls back to `curl`; the cargo error text mentions
`cl.exe/link.exe` though Ubuntu uses `cc`; Docker writes root-owned files, but
they are readable and the staging dir is runner-owned, so later moves/deletes
succeed. I could not execute it under pwsh 7 here (this machine has only 5.1);
the Monaco dry run is the first real test.

## Caching and cost

Cache **Rust only**: `~/.cargo/{registry,git}` and both `target/` dirs; 10 GB
per repo, 7-day eviction, saves a 2–5 minute cold compile
([caching](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)).
Skip Docker tarball caching (~2 GB restore is no faster than a pinned pull) and
PBF caching for scheduled builds (`-latest` changes daily; cache only same-day
retries keyed by SHA). Public-repo minutes are free; releases are free, **2 GiB
per asset**, no bandwidth cap
([releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases));
all three assets fit under 2 GiB at z14 by estimate.

## Failure modes

1. **Disk exhaustion — the real risk.** Ukraine peaks at 10–15 GB against 14.
   Reclaim disk (pin the action by SHA), use a larger/self-hosted runner, guard
   with `df -h`, and drop the Planetiler image after the basemap step.
2. **Docker pulls.** Hosted public pulls are exempt, but an outage or deleted
   digest still fails the job; retry and mirror to ghcr for self-hosted.
3. **Geofabrik `-latest` drift.** One download per build, so no mid-build mix;
   a hash mismatch fails safely. But daily republication (~21:00 CET,
   [Geofabrik](https://download.geofabrik.de/technical.html)) means a weekly
   pin fails: today's `moldova-latest` is 101,528,306 B, while the pinned
   101,528,058 B is the size of the dated `moldova-261115.osm.pbf`. Use dated
   URLs or refresh the pin; Geofabrik deletes old files, so mirror for
   long-term rebuilds.
4. **6 h cap.** No region is close (<2 h estimated); add `timeout-minutes` so a
   hung pull or Planetiler run fails fast.
5. **Placeholder URLs.** `regions.json` ships `packs.pinpoint.example`; pass
   `-BaseUrl https://github.com/.../releases/download/data-<region>-<date>` or
   publish dead links (`verify.ps1` only rejects empty URLs).

## Recommendation

1. First dry run: `pwsh -NoProfile -File scripts/offline/build.ps1 -Region monaco -DryRun`
2. Then the Monaco fixture end-to-end: `pwsh -NoProfile -File scripts/offline/build.ps1 -Region monaco -SkipDownload -SkipRouting -OsmPath scripts/offline/tests/fixtures/monaco.osm.pbf`
3. Then Moldova in full. **Moldova and Romania are GO** on `ubuntu-latest`;
   **Ukraine is NO-GO as-is** (the basemap phase peaks at 10–15 GB against 14)
   and becomes GO with a disk-reclaim step, larger runner, or self-hosted
   ≥40 GB free.

Add Romania and Ukraine to `regions.json` (bbox, maxzoom, SHA-256) before they
can run, and refresh the Moldova pin — it is already stale.

Sources: [build.ps1](../../scripts/offline/build.ps1), [moldova.md](moldova.md),
[design](../superpowers/specs/2026-11-17-global-maps-design.md),
[native-routing.md](native-routing.md); PBF sizes are HTTP `Content-Length`
from `download.geofabrik.de` on 2026-11-17.
