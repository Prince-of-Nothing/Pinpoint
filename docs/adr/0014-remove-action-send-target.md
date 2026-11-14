# 0014. Remove the ACTION_SEND target until an onNewIntent bridge exists

Date: 2026-11-14

## Status

Accepted

## Context

Pinpoint 1.3.4/1.3.5 registered an `ACTION_SEND` `text/plain` intent filter so
other apps could share a map link or coordinates into Pinpoint. The first
share worked when it launched a cold process.

Android delivers further shares to the existing activity's `onNewIntent`.
`NativeActivity` does not call `setIntent`, and the Rust app only reads the
launch intent, so a second share was silently ignored while an instance was
alive. Live testing confirmed this (`getIntent` did not change after
`onNewIntent`).

Two Rust-only alternatives were evaluated and rejected:

- **`launchMode=standard`** makes each share start another native app instance
  in the same process. It handled three consecutive shares, but
  `android-activity` then logged
  `Ignoring spurious JVM callback after last activity reference was dropped`,
  threads stayed elevated, and multiple eframe/tokio instances remained. Not
  safe to ship.
- **Polling `getIntent()`** cannot work because `onNewIntent` does not update
  the activity's intent.

A correct fix needs a small Java `NativeActivity` subclass overriding
`onNewIntent` and calling `setIntent`, packaged as a prebuilt DEX with a
manifest component. That breaks the Rust-only packaging rule (ADR-0007) and
needs an explicit product decision.

## Decision

Remove the `ACTION_SEND` intent filter from the Android manifest. Pinpoint will
not appear in the system text-share sheet until an `onNewIntent` bridge is
approved and implemented. Keep share-out (places and kiosk links) and the
`geo:`/`pinpoint://` VIEW deep links.

Retain the shared-text parser and JNI reader in the codebase as the receiving
half of a future bridge; they are unit-tested and currently unused at runtime.

## Consequences

- No share destination that silently no-ops. Users lose share-in until the
  bridge exists.
- The manifest contains only `VIEW` filters (`pinpoint`, `geo`) and the normal
  launcher intent.
- Re-enabling share-in requires: a Java `onNewIntent` subclass (or an
  equivalent android-activity API), `setIntent` + `NativeActivity` result
  forwarding, the manifest filter, and a device test that shares twice without
  force-stopping.
