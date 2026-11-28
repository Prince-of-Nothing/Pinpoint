# Plan: keep the fallback keychain master key out of the app-data folder

Status: **follow-up plan, not implemented.** This is finding F-10 in
[threat-model-publish-readiness.md](threat-model-publish-readiness.md).

## What happens today

`open_default` (`src/security/keychain.rs:213-223`) tries the OS keychain first
(`OsKeychain`, backed by the `keyring` crate). When that is unavailable and an
app-data directory is writable, it falls back to `FileKeychain`:

- `load_or_create_master_key` (`src/security/keychain.rs:227`) generates a
  256-bit key once and writes it to `dir/master.key`. On Unix the file is made
  owner-only, but it is still a plain file.
- `FileKeychain` (`src/security/keychain.rs:51-116`) encrypts each entry with
  that master key and stores the ciphertext as files in the same `dir`.

Anyone who can read the app-data folder therefore gets the key and the
encrypted entries together, which makes the encryption decorative. The file
lives beside the data it protects.

The fallback is used on headless Linux and on platforms with no native backend.
Desktop Windows and macOS normally take the native path, but the code does not
stop the fallback from being used there.

## Goal

The master key must never sit in the same folder as the entries it protects.
On each platform it is held by something the app user cannot copy off the
device with a file copy: Android Keystore, Windows DPAPI, the macOS and Windows
credential stores, or a passphrase the user types.

## Plan

**1. Remove the silent fallback where a native store exists.**
On Android, Windows and macOS, refuse to use `FileKeychain` when the native
store fails, and report the error. A user should never fall into the plain-file
mode without knowing it.

**2. Android: Keystore.**
Create a non-exportable AES key in Android Keystore through the existing Java
bridge (`android/app/src/main/java`, ADR-0017). Wrap the entries' master key
with it, and store only the wrapped form in app data. This needs one JNI
function pair: wrap and unwrap.

**3. Windows: DPAPI.**
Protect the fallback master key with `CryptProtectData` in user scope, and store
the protected blob, not the raw key. The blob can be decrypted only by the same
Windows user on the same machine.

**4. Headless Linux: no plain key.**
Prefer the Secret Service when a session bus has one. Otherwise derive the key
from a passphrase with Argon2id, which the app already uses for the PIN lock
(`src/app_lock.rs`), and ask for it at launch. The decision on whether headless
Linux should prompt or refuse is open; see the questions below.

**5. Migration.**
On first start with the new code, read the old `master.key`, re-wrap every
entry under the new scheme, verify each entry decrypts, then delete `master.key`.
If any step fails, keep the old file and report the error. Deleting the file
on this device is a local cleanup, not a remote action.

**6. Tests.**
- A test that after migration no file in the app-data folder contains the raw
  master key.
- The existing `file_keychain_encrypts_at_rest` test stays, as a check that
  entries are not stored in plain text.
- Platform checks on CI: a Windows job for DPAPI and an Android emulator run for
  Keystore, following the existing smoke-test scripts.

## Questions for the owner

- On headless Linux, should the app prompt for a passphrase at launch, or refuse
  to store secrets without a Secret Service?
- Is a Windows DPAPI blob acceptable as the fallback, or should Windows refuse
  to run without Credential Manager?

## Not in scope

- Changing what is encrypted or the entry format.
- Server-side key handling; the server never sees the master key.
