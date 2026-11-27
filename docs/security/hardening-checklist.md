# Deployment and infrastructure hardening checklist

Tracks the work needed to harden the deployment stack, grouped by area. Each item is a task to complete, not a description of a weakness. Tick an item when its change is merged and verified.

## Network exposure

- [ ] Stop publishing database, admin and diagnostics ports on the host.
- [ ] Serve the public gateway over HTTPS only, with a redirect from HTTP and HSTS.
- [ ] Reach the admin API only over a private network or tunnel, not through the public gateway.
- [ ] Split the compose network: an internal backend network for the database and services, and a separate edge network for the gateway.
- [ ] Refuse to serve plain HTTP on a non-loopback address unless explicitly enabled.

## Secrets and configuration

- [ ] Read every database password and key from the environment, with no literal defaults.
- [ ] Refuse to start without the column-encryption key outside development profiles.
- [ ] Remove unused settings from the example environment file, or wire them up.
- [ ] Set the production session idle default to 30 minutes.
- [ ] Give the admin service its own database role.

## Database

- [ ] Apply least-privilege grants automatically at first boot, and verify they were applied.
- [ ] Remove access to credential and session tables from roles that do not need it.
- [ ] Make the audit table append-only for the application role.
- [ ] Require TLS for database connections that leave the local host.

## Application controls

- [ ] Key rate limits on the socket peer address, and trust forwarded headers only from configured proxies.
- [ ] Fail closed when no client address is available.
- [ ] Share rate-limit counters across replicas.

## Containers

- [ ] Pin base images by digest.
- [ ] Run containers with a read-only root filesystem, dropped capabilities, `no-new-privileges`, and memory and process limits.
- [ ] Set log size limits on every service, and reduce per-request logging.
- [ ] Add a CI job that builds every image and runs the compose health checks.
- [ ] Add the Docker ecosystem to Dependabot.
- [ ] Exclude `.env*`, key and certificate files from `.dockerignore`.

## Backups and recovery

- [ ] Replace the plaintext periodic dumps with encrypted, scheduled, off-host backups.
- [ ] Rehearse a restore and record the result.
- [ ] Align the incident plan's restore steps with the backup process.

## Release and CI

- [ ] Put release signing behind a protected environment with required reviewers, limited to version tags.
- [ ] Pin GitHub Actions to commit SHAs.
- [ ] Pin the ZAP image, and stop running it as root with host networking.
- [ ] Lock Playwright dependencies in CI.
- [ ] Remove `actions: write` from the Windows release job.

## Operations and detection

- [ ] Publish a monitored security contact address in `CONTRIBUTING.md`, and add a `security.txt`.
- [ ] Add alert rules for authentication failures and rate-limit spikes.
- [ ] Ship logs and audit rows to a store the application cannot edit.
