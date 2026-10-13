# 1. Record architecture decisions

Date: 2026-09-21

## Status

Accepted

## Context

As LocationSharingApp's architecture takes shape, decisions need to be made about
technology choices, data handling, and system structure. Without a durable record,
the reasoning behind these decisions is easily lost — especially in an open-source
project where contributors change over time.

## Decision

We will use Architecture Decision Records, as described by Michael Nygard, to record
all architecturally significant decisions made in this project. Records are kept in
[`docs/adr/`](./README.md), one immutable file per decision, indexed from
[`do../architecture/architecture-decisions-link.md`](../architecture/architecture-decisions-link.md).

## Consequences

- Every significant decision has a discoverable, permanent record of its context and
  rationale, not just its outcome.
- Decisions that change over time are handled by superseding, not editing history, so
  the evolution of the architecture remains traceable.
- Contributors must remember to write an ADR for qualifying decisions; this is called
  out in [`docs/adr/README.md`](./README.md) and should be reinforced in code review.
