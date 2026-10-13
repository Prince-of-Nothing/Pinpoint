# Always Together — Documentation

This repository contains the complete technical documentation for **Always Together**, a consent-based location-sharing application designed for safety and real-world connection.

## Purpose and Audience

This documentation serves as **assessment evidence** for university evaluation. It demonstrates:
- Engineering decision-making under constraints (GDPR compliance, child safety, battery limitations)
- Security-first architecture for sensitive location data
- Traceable requirements from user needs through implementation
- Professional documentation practices matching industry standards

This is not intended as a learning resource for external users. The structure exists to provide graders with clear, auditable evidence of technical competence.

## Documentation Structure

The documentation follows three complementary frameworks chosen for their fit with high-stakes, compliance-heavy projects:

### Diátaxis Framework (4 Modes)

| Mode | Purpose | Location |
|------|---------|----------|
| **Tutorials** | Learning-oriented lessons for team onboarding | [`tutorials/`](./tutorials/) |
| **How-to Guides** | Task-oriented recipes for specific goals | [`how-to/`](./how-to/) |
| **Reference** | Factual specifications (APIs, data models, requirements) | [`reference/`](./reference/) |
| **Explanation** | Design rationale, context, and trade-off analysis | [`explanation/`](./explanation/) |

### arc42 Template (Architecture)

Standard 12-section architecture documentation covering goals, constraints, building blocks, runtime, deployment, security, quality requirements, and risks. Located in [`architecture/`](./architecture/).

### Architecture Decision Records (ADRs)

Immutable, numbered records preserving the reasoning behind significant architectural choices. Located in [`adr/`](./adr/).

## Why This Structure

A 20-year engineering veteran would recognize this pattern: **structure matches stakes**. 

For a fast prototype with no sensitive data? Lightweight docs work fine. For Always Together — where privacy failures enable stalking, consent bugs breach GDPR, and location leaks endanger users — the overhead is justified:

| Benefit | Why It Matters Here |
|---------|---------------------|
| **Traceability** | GDPR Article 5 requires documented purpose limitation. Every data field traces back to a requirement. |
| **Audit Readiness** | Child safety regulations demand clear consent flows. Auditors can follow REQ → API → DB → Test. |
| **Decision Durability** | ADRs prevent "why did we choose this?" conversations after team changes or refactors. |
| **Security Clarity** | Threat models, controls, and incident response are explicit, not scattered in chat logs. |
| **Mentor Review** | Internship evaluators need visible engineering rigor, not just working code. |

### Trade-offs Acknowledged

This approach costs more upfront. That's acceptable because:
- The project has regulatory constraints (GDPR, UK Children's Code)
- Failure modes include real-world harm (stalking, harassment)
- University assessment rewards demonstrated process, not just outcomes
- Security-critical systems benefit from explicit rationale

Compare to Bluesky's informal docs: their approach works for protocol experimentation with technical users. Our approach fits safety-critical apps with non-technical stakeholders and compliance requirements.

## Quick Navigation for Assessment

| Evidence Category | Primary Documents |
|-------------------|-------------------|
| **Product Vision** | [Introduction & Goals](./architecture/introduction-and-goals.md), [Scope & MVP](./architecture/scope-and-mvp-boundaries.md) |
| **Requirements Engineering** | [MVP Requirements](./reference/mvp-requirements-and-acceptance-criteria.md), [Traceability Matrix](./reference/requirements-traceability-matrix.md) |
| **Architecture Design** | [Architecture Models](./architecture/architecture-models.md), [Building Blocks](./architecture/building-block-view.md), [Deployment](./architecture/deployment-view.md) |
| **Security Engineering** | [Security Architecture](./architecture/security-architecture-and-data-protection.md), [Threat Controls](./architecture/security-threats-and-controls.md), [Security Control Register](./reference/security-control-register.md) |
| **Data Protection** | [Privacy & Data Handling](./architecture/privacy-and-data-handling.md), [Domain Data Model](./reference/domain-data-model.md) |
| **API Design** | [API Contract](./reference/api-contract.md), [API Security Implementation](./reference/api-security-implementation.md) |
| **Research & Validation** | [Pre-Implementation Validation](./explanation/pre-implementation-validation.md), [Research Notes](./explanation/research-notes-and-references.md) |
| **Decision History** | [ADR Index](./architecture/architecture-decisions-link.md) |
| **Project Planning** | [Development Roadmap](./explanation/development-roadmap-and-evidence.md) |

## Design Artifacts

**Figma UI/UX Designs**: [Always Together Prototypes](https://www.figma.com/design/BqFYN5OtLLti55TAtB8xPX/Untitled?node-id=0-1&t=HoAPPUeyP4vFTEoP-0)

Wireframes, user flows, and visual designs complementing the technical documentation. Key screens also documented in [UI Navigation & Wireframes](./reference/ui-navigation-and-wireframes.md).

### Embedding Figma Images

For detailed visual reference, export relevant frames from Figma and place them in the [`assets/`](./assets/) directory:

```markdown
![Login Flow](./assets/ui-flows/login-sequence.png)
*Figure: User authentication flow from onboarding to verified account.*
```

See [`assets/README.md`](./assets/README.md) for conventions on exporting from Figma, naming files, and embedding images in documentation.

## Future Community Feedback

*Coming soon:* A Discord server will be created for external feedback and suggestions. Link will be added here once available.

## Documentation Layout

```
docs/
├── README.md                 # this file
├── CONTRIBUTING.md           # documentation standards and review process
│
├── tutorials/                # Learning-oriented onboarding
├── how-to/                   # Task-oriented guides
├── reference/                # APIs, data models, requirements
├── explanation/              # Design rationale and research
│
├── architecture/             # arc42 template (12 sections)
│   ├── introduction-and-goals.md
│   ├── scope-and-mvp-boundaries.md
│   ├── constraints.md
│   ├── context-and-scope.md
│   ├── solution-strategy.md
│   ├── architecture-models.md
│   ├── building-block-view.md
│   ├── runtime-view.md
│   ├── deployment-view.md
│   ├── crosscutting-concepts.md
│   ├── privacy-and-data-handling.md
│   ├── security-architecture-and-data-protection.md
│   ├── security-threats-and-controls.md
│   ├── incident-response-plan.md
│   ├── location-sharing-lifecycle.md
│   ├── movement-trails-and-search-corridors.md
│   ├── friend-mosaic-maps.md
│   ├── cache-and-permission-model.md
│   ├── messaging-and-group-lifecycle.md
│   ├── architecture-decisions-link.md
│   ├── quality-requirements.md
│   ├── risks-and-technical-debt.md
│   ├── glossary.md
│   └── diagrams/             # C4 model diagrams (Mermaid)
│
└── adr/                      # Architecture Decision Records
    ├── template.md
    ├── record-architecture-decisions.md
    └── adopt-always-together-product-direction.md
```

## Maintaining Documentation Quality

1. **ADRs precede implementation** — Significant decisions are recorded before or during code changes, never retroactively.

2. **Architecture updates ship with code** — When the system changes, relevant arc42 sections update in the same PR.

3. **Superseded ≠ deleted** — Old ADRs remain immutable; new ADRs reference and supersede them.

4. **Diagrams as code** — All diagrams use Mermaid/PlantUML for version control and diff review.

5. **No documentation debt** — "Update docs later" is not an acceptable PR comment.

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for detailed standards, style conventions, and audit procedures.
