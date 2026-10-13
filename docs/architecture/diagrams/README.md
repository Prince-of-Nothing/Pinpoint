# Architecture Diagrams

Diagrams here follow the **C4 model** levels and are written as text (Mermaid) so they are diffable and reviewable in pull requests, and never silently drift out of sync with the code.

## C4 Model Levels

| Level | File | Purpose |
|-------|------|---------|
| 1. Context | `context-diagram.md` | System + external actors/systems ([§3](../context-and-scope.md)) |
| 2. Container | `container-diagram.md` | Deployable/runnable units ([§5](../building-block-view.md)) |
| 3. Component | `component-*.md` | Internals of a single container ([§5.2](../building-block-view.md#52-level-2-component-breakdown)) |
| Runtime | `sequence-*.md` | Key runtime scenarios ([§6](../runtime-view.md)) |
| Deployment | `deployment-diagram.md` | Physical/infra topology ([§7](../deployment-view.md)) |

## Getting Started

Use the example below as a starting template for `context-diagram.md`:

```mermaid
C4Context
    title System Context diagram for LocationSharingApp

    Person(user, "User", "A person sharing their location with friends")
    System(app, "LocationSharingApp", "Lets users share real-time location with friends")
    System_Ext(push, "Push Notification Service", "Delivers alerts to mobile devices")
    System_Ext(maps, "Map Provider", "Renders maps and geocoding")

    Rel(user, app, "Shares location, views friends' locations")
    Rel(app, push, "Sends notifications via")
    Rel(app, maps, "Requests map tiles / geocoding from")
```

## Adding Diagrams

Add real diagrams here as the architecture is defined, and keep this README's table up to date with the files that actually exist.

**Tips:**
- Use Mermaid for sequence diagrams and simple structure diagrams
- Use PlantUML for complex component diagrams if needed
- Keep diagram titles descriptive
- Reference diagrams from the main architecture documents
