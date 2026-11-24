# Design Patterns

The security-critical paths are small and explicit, but several recurring
structures follow documented design patterns. `src/patterns/` holds compact
reference implementations with **23 unit tests**; the table also names the
production code that uses the same idea, so the pattern is not only a teaching
artifact.

| Pattern | Category | Reference implementation | Where the app uses it | Why it matters here |
|---|---|---|---|---|
| Repository | Structural | `src/patterns/repository.rs` (traits for users, friendships, shares, locations) | `src/server/state/store.rs` and the `state/users.rs`, `state/locations.rs`, `state/sessions.rs` load/persist code | Data access is behind one module per table; handlers never build SQL strings, and tests drive the same router with an in-memory store |
| Strategy | Behavioural | `src/patterns/strategy.rs` (`PrecisionStrategy`: exact/approximate/city-only) | Precision is applied in `src/server/locations_handlers.rs` before serialization | A consent decision is one swap of a strategy, not a fork in every handler; the precision is validated at the boundary |
| Command | Behavioural | `src/patterns/command.rs` (`LoginCommand`, `PublishLocationCommand`, `CreateShareCommand`) | `PendingAction` in `src/app.rs` / `src/ui.rs` | User actions become values the async runner executes, so the UI never performs network work on the render thread |
| State | Behavioural | `src/patterns/state.rs` (`ScreenState`, `StateContext`) | `StateContext` is used by `src/app.rs`; `CurrentScreen` drives the shell | Screen transitions are explicit, which keeps the login/registration secret-clearing and the expired-session reset in one place |
| Observer | Behavioural | `src/patterns/observer.rs` (`EventSubject`, `Observer`, `AppEvent`) | Refresh-on-change callbacks (`onChanged: refresh`) and the polling loops | One change can refresh the map, lists and badges without the producers knowing the consumers |
| Factory | Creational | `src/patterns/factory.rs` (`ApiClientFactory`) | `build_backend`/`build_default_backend` in `src/backend/mod.rs` | The REST, failover and browser adapters are selected once at startup; the app targets `DynBackend` |
| Decorator | Structural | `src/patterns/decorator.rs` (`LoggingApiClient<C>`) | `FailoverBackend` in `src/backend/failover.rs` wraps several backends with retry/failover | Cross-cutting behaviour is added around a backend without changing the handler code or the security checks |
| Proxy | Structural | `src/patterns/proxy.rs` (`ProxyLocationService`) | Offline mode and `FailoverBackend` sit between the UI and the network | A request can be served from cache/offline or retried on another replica while the caller sees one interface |

## How to read this

- The reference modules are intentionally small and tested; they are not a
  second implementation of the app.
- When a pattern touches a security rule (Repository, Strategy, State), the
  controlling test is named in the
  [control register](./security-control-register.md), not in this page.
- Adding a new pattern is not a goal by itself: the course asks for
  explainable structure, not the largest catalogue.
