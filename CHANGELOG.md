# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
Semantic Versioning. This is a pre-1.0 (thesis MVP) project.

## [Unreleased]
### Added
- Kotlin/Jetpack Compose Android app scaffold (`mobile/`) — planned next
- `weba` admin console and `webk` kiosk check-in clients — planned
- Project governance docs: CONTRIBUTING, CODE_OF_CONDUCT, docs/{PROJECT, ROADMAP,
  ARCHITECTURE, SECURITY, PRIVACY, TESTING, DEPLOYMENT, GLOSSARY}, LICENSE template

### Changed
- _(open)_ —

## [0.3.0] - 2026-10-13
### Added
- Docker deployment: `docker-compose.yml`, multi-stage `backend/Dockerfile` (non-root,
  healthcheck), `web/Dockerfile` + `web/nginx.conf` (SPA + `/api` reverse proxy)
- Idempotent demo seeding (`SEED_DEMO_DATA`): admin (forced password change), Alice & Bob
  with mutual shares and Paris locations
- Static file serving from the Go binary when `WEB_ROOT` is set (single-container option)
- `COOKIE_INSECURE` dev flag; `.env.example` documenting all settings
- Web SPA v1: login/CSRF flow, OSM map with friend pins, freshness/stale badges, precision
  badges, navigation export, location publishing, friend requests, directional share
  management, admin provisioning, forced-password-change UX

### Changed
- Mobile client decision: Flutter → **Kotlin (Android-first)** due to no macOS/iPhone
  development hardware; Android emulator covers the full test loop

## [0.2.0]
### Added
- Full Go backend feature parity: JWT + session auth, bcrypt, CSRF double-submit,
  rate limiting, lock/unlock, must-change-password gate
- Authorization + security test suite (`go test ./...` green, incl. IDOR fail-closed cases)

### Removed
- Entire legacy Python/FastAPI backend (refactor complete; no `.py` files remain)

## [0.1.0]
### Added
- Initial Python/FastAPI prototype (superseded by 0.2.0)
- Core domain model: users, friendships, directional share permissions, current location
