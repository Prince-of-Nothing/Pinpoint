# Grader's Quick Reference

This document provides university evaluators with a quick reference for assessing the Always Together project. Use this alongside the [Development Roadmap and Evidence](../explanation/development-roadmap-and-evidence.md) for comprehensive evaluation.

## Assessment Criteria Mapping

### 1. Requirements Engineering (20%)

**Evidence Locations:**
- [MVP Requirements and Acceptance Criteria](../reference/mvp-requirements-and-acceptance-criteria.md) — REQ-01 through REQ-09
- [Requirements Traceability Matrix](../reference/requirements-traceability-matrix.md) — Full traceability chain
- [Introduction and Goals](../architecture/introduction-and-goals.md) — Problem statement and stakeholder needs

**What to Look For:**
- Clear, testable acceptance criteria for each requirement
- Traceability from user needs → API → database → tests
- Explicit non-goals preventing scope creep
- GDPR compliance baked into requirements (not retrofitted)

### 2. Architecture Design (25%)

**Evidence Locations:**
- [Architecture Models](../architecture/architecture-models.md) — C4 diagrams
- [Building Block View](../architecture/building-block-view.md) — Component decomposition
- [Deployment View](../architecture/deployment-view.md) — Infrastructure layout
- [Security Architecture](../architecture/security-architecture-and-data-protection.md) — Security boundaries and controls

**What to Look For:**
- Standard arc42 structure covering all 12 sections
- Security-first design with explicit threat modeling
- Separation of concerns (mobile, API, database, cache)
- Scalability considerations documented

### 3. Security Engineering (25%)

**Evidence Locations:**
- [Security Control Register](../reference/security-control-register.md) — SC-01 through SC-09
- [API Security Implementation](../reference/api-security-implementation.md) — OAuth 2.0/PKCE, authorization sequences
- [Security Threats and Controls](../architecture/security-threats-and-controls.md) — Threat-first analysis
- [Incident Response Plan](../architecture/incident-response-plan.md) — Severity levels and response stages

**What to Look For:**
- OAuth 2.0 with PKCE (RFC 7636) implementation
- Relationship-based access control with block checking
- Precision transformation (exact vs. approximate location)
- Audit logging for privacy-sensitive operations
- Token rotation and expiry enforcement

### 4. Data Protection & Privacy (15%)

**Evidence Locations:**
- [Privacy and Data Handling](../architecture/privacy-and-data-handling.md) — GDPR alignment
- [Domain Data Model](../reference/domain-data-model.md) — Entity relationships
- [Cache and Permission Model](../architecture/cache-and-permission-model.md) — Location-bound content

**What to Look For:**
- Purpose limitation documented per data field
- Consent lifecycle (grant, active, expire, revoke)
- Data minimization in API responses
- Right to deletion implemented
- Child safety considerations (UK Children's Code alignment)

### 5. Implementation Quality (15%)

**Evidence Locations:**
- [Testing and Quality Strategy](../how-to/testing-and-quality-strategy.md) — Test coverage expectations
- [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md) — Technology choices with rationale
- [API Contract](../reference/api-contract.md) — Versioned API specification

**What to Look For:**
- Frozen technology stack with clear rationale
- API versioning strategy (`api/v1`)
- Error handling following RFC 7807 (Problem Details)
- Test pyramid coverage (unit, integration, security, accessibility)

## Quick Demo Script

For live demonstration (10-15 minutes):

### Minute 0-2: Setup Verification
```bash
curl http://localhost:8000/api/v1/health
# Shows: healthy, database connected, redis connected
```

### Minute 2-5: Authentication Flow
1. Open mobile app
2. Show OAuth 2.0 login screen
3. Demonstrate PKCE flow (show code challenge in network tab)
4. Display received JWT token structure

### Minute 5-8: Core Feature — Location Sharing
1. Send friendship request (show API call)
2. Accept friendship on second account
3. Create location share with:
   - Selected recipient
   - Precision level (approximate)
   - Purpose (safety)
   - Expiry (2 hours)
4. Show recipient's map view with obfuscated location
5. Revoke share and show immediate access loss

### Minute 8-10: Safety Features
1. Create scheduled check-in
2. Show reminder notification
3. Complete check-in
4. Demonstrate what happens when check-in is missed (grace period → alert)

### Minute 10-12: Privacy Controls
1. Navigate to privacy settings
2. Change default precision from exact to approximate
3. Show relationship-based profile visibility:
   - Self view (full data)
   - Friend view (no precise location)
   - Non-friend view (empty profile)
4. Demonstrate blocking a user

### Minute 12-15: Audit Trail
```bash
docker-compose exec postgres psql -U postgres -d always_together \
  -c "SELECT event_type, user_id, timestamp FROM audit_logs ORDER BY timestamp DESC LIMIT 10;"
```
Show privacy-preserving access logs demonstrating:
- Location share creation events
- Access events with friend count (not individual IDs)
- Token refresh events

## Key Questions for Discussion

### Architecture Decisions
1. **Why Flutter over React Native?** — See [Technology Stack](../reference/technology-stack-and-infrastructure.md)
2. **Why OAuth 2.0 with PKCE instead of simple JWT auth?** — See [API Security Implementation](../reference/api-security-implementation.md)
3. **Why PostgreSQL over NoSQL for location data?** — See [Building Block View](../architecture/building-block-view.md)

### Security Trade-offs
1. **How do you prevent stalking via location sharing?** — Mutual friendship + directional consent + block functionality
2. **What happens if a token is compromised?** — 15-minute expiry + device binding + rotation on refresh
3. **How do you ensure approximate location is truly approximate?** — Server-side transformation before response

### Compliance Alignment
1. **Which GDPR articles are addressed?** — See [Privacy and Data Handling](../architecture/privacy-and-data-handling.md)
2. **How does this comply with UK Children's Code?** — Age-gating, default privacy, best interest of child
3. **What data is retained and for how long?** — See [Domain Data Model](../reference/domain-data-model.md)

## Common Weaknesses to Avoid

❌ **Vague requirements** — All requirements have explicit acceptance criteria  
❌ **Missing threat model** — Security threats documented before controls  
❌ **No decision history** — ADRs preserve rationale for significant choices  
❌ **Retroactive documentation** — Docs updated alongside code, not after  
❌ **Compliance as afterthought** — GDPR/child safety embedded in requirements  

## Marking Rubric Alignment

| Grade | Characteristics |
|-------|-----------------|
| **First (70%+)** | All evidence complete, exceeds MVP scope, exceptional security rigor, clear trade-off analysis |
| **Upper Second (60-69%)** | MVP complete, good security practices, solid documentation, minor gaps in edge cases |
| **Lower Second (50-59%)** | Core features working, basic security, documentation present but incomplete |
| **Third (40-49%)** | Partial implementation, security concerns unaddressed, minimal documentation |
| **Fail (<40%)** | Non-functional, critical security vulnerabilities, no documentation |

## Contact for Clarification

For questions about specific documentation sections or implementation details, refer to:
- Architecture questions → [`architecture/`](../architecture/) directory
- API questions → [`reference/api-contract.md`](../reference/api-contract.md)
- Security questions → [`reference/security-control-register.md`](../reference/security-control-register.md)
- Decision rationale → [`adr/`](../adr/) directory

---

*This reference complements the full [Development Roadmap and Evidence](../explanation/development-roadmap-and-evidence.md) document.*
