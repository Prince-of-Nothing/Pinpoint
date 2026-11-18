# Artifact review - v2.6.1 release and APK

**Checked:** 2026-11-18 10:48 +02:00, branch `docs/midterm2` (HEAD `5285bc6b`).
**Scope:** GitHub release `v2.6.1`, release asset, local APK, and the artifact
claims in `Midterm2-Report.md` and `Presentation.md`.
**Verdict:** all cited numbers / names / fingerprints are **correct**. Two
wording-precision corrections are suggested; no value needs changing.

## Tools

- `gh` CLI against `Prince-of-Nothing/Pinpoint`.
- Android SDK build-tools **36.0.0** at
  `C:\Users\Dinusauuuur\AppData\Local\Android\Sdk\build-tools\36.0.0`
  (`aapt`, `apksigner`).
- APK under test:
  `C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\pinpoint-android-2.6.1.apk`
  (pre-existing, not re-downloaded here).

## Release identity

`gh api repos/Prince-of-Nothing/Pinpoint/releases/latest`:

```json
{"draft":false,"html_url":"https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1",
 "name":"Pinpoint 2.6.1","prerelease":false,"published_at":"2026-11-18T08:28:49Z",
 "tag_name":"v2.6.1","target_commitish":"main"}
```

- It is the **latest** release (also marked `Latest` by `gh release list`).
- Annotated tag `v2.6.1` = `3e48e1d7f295ffe21185b80eb0940a2ef39cad8d`, deref
  commit `345aa37f27424c46883a579e7badd257e075227f`; local and `origin` tags agree.
- Assets (2): `pinpoint-android-2.6.1.apk` (13,632,525 B,
  `application/vnd.android.package-archive`, digest
  `sha256:402b5c7614eab92babc735a3efa465161686363ece2c06df2deaa033ddaabc2e`)
  and `pinpoint-android-2.6.1.apk.idsig` (117,194 B). No other APK assets.

## APK file identity

```text
Length : 13632525
SHA256 : 402b5c7614eab92babc735a3efa465161686363ece2c06df2deaa033ddaabc2e
```

The local file is **byte-identical** to the published asset (size + SHA-256
match the GitHub asset digest). Local mtime 2026-11-18 10:28:22 +02:00 is
consistent with publication at 08:28:49Z.

## Manifest (`aapt dump badging`)

```text
package: name='com.pinpoint.desktop' versionCode='20601' versionName='2.6.1'
sdkVersion:'26'
targetSdkVersion:'35'
application-label:'Pinpoint'
launchable-activity: name='com.pinpoint.desktop.PinpointActivity'
native-code: 'arm64-v8a' 'armeabi-v7a'
```

`versionCode` is reproducible from the build: `Cargo.toml` version `2.6.1`
gives `2*10000 + 6*100 + 1 = 20601` via
`android/app/build.gradle.kts:14-17`.

Native libraries inside the APK (ZIP listing, complete `lib/` tree):

```text
lib/arm64-v8a/libpinpoint_desktop.so     21442296 B
lib/armeabi-v7a/libpinpoint_desktop.so   15641040 B
```

Two ABIs, exactly as claimed; no x86/x86_64 libraries. The release workflow
ships this universal build with `-Pabi=universal`
(`.github/workflows/release.yml:127-132`).

## Signature (`apksigner verify --verbose --print-certs`)

```text
Verifies
Verified using v1 scheme (JAR signing): false
Verified using v2 scheme (APK Signature Scheme v2): true
Verified using v3 scheme (APK Signature Scheme v3): true
Verified using v3.1 scheme (APK Signature Scheme v3.1): false
Verified using v4 scheme (APK Signature Scheme v4): false
Number of signers: 1
Signer #1 certificate DN: CN=Pinpoint, O=Pinpoint, C=MD
Signer #1 certificate SHA-256 digest: 2b326ca140f5d34ac61672fb3a5ec2596ced91f9a2fd618d50b52fe89dea7e49
Signer #1 certificate SHA-1 digest: d4ed97061962a791e116fb10ba1e1f20ee4984db
Signer #1 certificate MD5 digest: d7ba0ca7c0992ec82691b9e343116333
Signer #1 key algorithm: RSA
Signer #1 key size (bits): 4096
Signer #1 public key SHA-256 digest: 621a338de49a7e5d280da49c6c07af6050180438936d3a12b2146b24708bcda4
```

`verify` exits `0`. The certificate is not the throwaway build key
(`CN=Pinpoint Build`); the release workflow re-signs every tagged APK with the
secret release keystore and rejects a tag signed with the throwaway key
(`.github/workflows/release.yml:138-156, 164-166`). The documented release
keystore uses `CN=Pinpoint, O=Pinpoint, C=MD` and RSA 4096
(`android/README.md:204-210`), matching the certificate above.

v1 JAR signing is absent; this is fine because `minSdkVersion` is 26 and v2 is
supported there. The `.idsig` asset is only the v4 sidecar; v4 is not embedded
in the APK - the docs make no claim about it.

## Claim comparison

| Claim | Source | Observed | Result |
|---|---|---|---|
| v2.6.1 is the release / latest | `Midterm2-Report.md:9,30` | latest, not draft/prerelease | OK |
| `pinpoint-android-2.6.1.apk` | `Midterm2-Report.md:261` | exact name | OK |
| Size 13,632,525 B | `Midterm2-Report.md:261` | 13,632,525 B | OK |
| versionName `2.6.1` | `Midterm2-Report.md:262` | `2.6.1` | OK |
| versionCode `20601` | `Midterm2-Report.md:262` | `20601` | OK |
| `arm64-v8a` + `armeabi-v7a` | `Midterm2-Report.md:85,262`; `Presentation.md:132-133` | exactly those two | OK |
| Release-key certificate SHA-256 `2b326ca1...e7e49` | `Midterm2-Report.md:263-264` | exact match | OK |
| APK signed and verified | `Presentation.md:144,153` | v2 + v3 verify, exit 0 | OK |
| 13.6 MB signed APK | `slides.html:269` | 13.63 MB decimal (13.0 MiB) | OK |
| versionCode `20601` | `slides.html:268` | `20601` | OK |

The same certificate fingerprint also appears in `docs/midterm2/Handout.md:106`
and matches. (Handout is outside the requested comparison, noted for consistency.)

## Corrections (wording only, no wrong values)

1. `Midterm2-Report.md:263` (and `Handout.md:106`) say the APK is "signed with
   the Pinpoint release key (SHA-256 `2b326ca1...`)". The quoted digest is the
   **signing certificate** SHA-256; the RSA public key SHA-256 is
   `621a338de49a7e5d280da49c6c07af6050180438936d3a12b2146b24708bcda4`.
   Suggest: "signed with the Pinpoint release certificate (SHA-256 ...)".
   Someone comparing a key fingerprint would otherwise get a mismatch.
2. `Presentation.md:132-133` say "The 2.6.1 APK is signed for arm64 and
   armeabi-v7a." The signature covers the whole APK; the ABIs come from the
   packaged native libraries. Suggest: "The 2.6.1 APK ships native libraries
   for arm64-v8a and armeabi-v7a and is v2/v3-signed."

No change is needed to the size, versionName, versionCode, ABI list or
fingerprint values; they match the published artifact exactly.

## Not verified here (out of scope)

- Live `/health` version `2.6.1` / PostgreSQL
  (`Midterm2-Report.md:265,378`); no live endpoint was queried.
- The 14/14 live API, 9/9 UI, 7/7 offline and CI claims
  (`Midterm2-Report.md:244-267`; `Presentation.md:141-153`).
- Install/launch on a physical or emulated device.
- Whether the private release keystore is still solely held by the owner
  (not provable from public data; the certificate matches the documented
  release identity).