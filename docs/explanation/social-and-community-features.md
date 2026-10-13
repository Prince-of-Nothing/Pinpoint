# Social and Community Features

These are deferred, Full-mode features layered on top of the safety-first MVP
(see [§1 Introduction and Goals](../architecture/introduction-and-goals.md)).
They are intentionally separate from location permissions and can be added
after the safety foundation is reliable.

## Group matching

Users can describe a goal, interest, preferred perspective, and availability.
The system may suggest a temporary group or skill exchange. Users can
decline, leave, remain pseudonymous, or create their own group.

## Creator groups

A creator can define a group's purpose, rules, membership, roles, moderation
actions, and content boundaries. Groups must support reporting, blocking,
removal, and clear ownership.

## Messaging and calls

Trusted messaging belongs in Full mode. Voice and video calls are later
additions and should respect the same block, report, consent, and privacy
controls as text chat. See
[Messaging and Group Lifecycle](../architecture/messaging-and-group-lifecycle.md).

## Random groups

Random-person discovery is separate from trusted location circles. It
requires age-appropriate controls, rate limits, moderation, abuse reporting,
and no automatic access to precise location.

## Friend challenges

Friends and group creators can create timed challenges around shared
interests. Challenges may be competitive, cooperative, creative,
location-based, or puzzle-based. They use invitations and a visible
participant roster, with acceptance, decline, leave, and block controls. See
[Friend Challenges and Group Prompts](./friend-challenges-and-group-prompts.md).

---
*Deferred (Full-mode) features — not in the September MVP. See
[§1.5 MVP Requirements Overview](../architecture/introduction-and-goals.md#15-mvp-requirements-overview).*
