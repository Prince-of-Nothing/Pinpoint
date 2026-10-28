# Pinpoint

**Always Together** — a privacy-first, consent-based location-sharing safety
app. Location is only ever shared through explicit, directional, revocable
consent — never public, never silent.

| | |
|---|---|
| **Product** | Consent-based location sharing, friendships, check-ins, stale-device awareness |
| **Backend** | FastAPI (Python 3.11+) · SQLAlchemy · PostgreSQL/SQLite · Redis · JWT + server-side sessions |
| **Docs** | arc42 + Diátaxis + ADRs in [`docs/`](./docs/README.md) |
| **Status** | MVP backend increment: auth, friendships, directional location sharing — implemented and tested |

## Repository layout

```
backend/
├── app/
│   ├── api/          # Routers + DI dependencies (auth, users, friendships, locations)
│   ├── core/         # Configuration (fail-safe in production), time utilities
│   ├── db/           # Engine, session factory, declarative base
│   ├── middleware/   # Security-headers middleware
│   ├── models/       # ORM: users, sessions, friendships, consent, locations, audit
│   ├── repositories/ # Data access (Repository pattern)
│   ├── schemas/      # Pydantic request/response validation
│   └── services/     # Facades, policies, token service, rate limiting, audit
├── tests/            # Security regression suite (pytest)
├── requirements.txt
└── .env.example
docs/                 # Full documentation set (see docs/README.md)
```

## Quickstart (backend)

```bash
cd backend
python -m venv .venv && .venv\Scripts\activate      # or: python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
copy .env.example .env                              # then set SECRET_KEY
uvicorn app.main:app --reload
```

No database at hand? Set `DATABASE_URL=sqlite:///./dev.db` in `.env` for a
zero-dependency local run. Interactive API docs are at `http://localhost:8000/docs`.

Run the security test suite:

```bash
pytest tests/ -v
```

## Security at a glance

- **Complete mediation** — every location read runs `LocationAccessPolicy`
  (accepted friendship + owner→viewer direction + valid consent + no block).
- **Consent is bounded** — shares carry purpose and precision, expire within
  24 h by default, and are revocable in one call; blocking revokes everything.
- **Anti-enumeration** — identical generic 401s and timing-equalised login;
  stranger profiles are indistinguishable from non-existent ones.
- **Token hygiene** — 15-minute JWTs (algorithm allowlist, required claims),
  session tokens stored only as hashes, token-version revocation on password
  change, idle + absolute session expiry, CSRF double-submit for cookie auth.
- **Hardened by default** — production refuses placeholder secrets and debug
  mode; security headers (HSTS/CSP/nosniff), trusted-host allowlisting,
  RFC 7807 errors with no internals, per-endpoint rate limits, PII-free audit
  events.

The design is structured around named object-oriented patterns; each control
maps to one implementation site:
[Design Patterns and Principles](./docs/reference/design-patterns.md) ·
[Security Control Register](./docs/reference/security-control-register.md) ·
[Diagrams (C4 + UML)](./docs/architecture/diagrams/README.md)

## Documentation

Start at [`docs/README.md`](./docs/README.md):

- **Architecture** (arc42): [Building Block View](./docs/architecture/building-block-view.md), [Runtime View](./docs/architecture/runtime-view.md), [Security Architecture](./docs/architecture/security-architecture-and-data-protection.md)
- **Reference**: [API Contract](./docs/reference/api-contract.md), [Design Patterns](./docs/reference/design-patterns.md), [Domain Data Model](./docs/reference/domain-data-model.md)
- **Decisions**: [ADR index](./docs/adr/README.md)

## License

See [LICENSE](./LICENSE).
