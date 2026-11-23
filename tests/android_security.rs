//! Android release-surface guard: source-level checks for the manifest and
//! network security policy, before a device ever sees the APK.

const MANIFEST: &str = include_str!("../android/app/src/main/AndroidManifest.xml");
const RELEASE_NETWORK: &str = include_str!("../android/res/xml/network_security_config.xml");
const DEBUG_NETWORK: &str =
    include_str!("../android/app/src/debug/res/xml/network_security_config.xml");

#[test]
fn release_manifest_disables_backup_and_cleartext() {
    for needle in [
        r#"android:allowBackup="false""#,
        r#"android:usesCleartextTraffic="false""#,
        r#"android:networkSecurityConfig="@xml/network_security_config""#,
    ] {
        assert!(MANIFEST.contains(needle), "manifest lacks {needle}");
    }
    assert!(
        !MANIFEST.contains(r#"android:usesCleartextTraffic="true""#),
        "cleartext must stay disabled in the manifest"
    );
}

#[test]
fn release_network_policy_allows_only_loopback() {
    assert!(RELEASE_NETWORK.contains(r#"cleartextTrafficPermitted="false""#));
    assert!(RELEASE_NETWORK.contains("<domain includeSubdomains=\"false\">127.0.0.1</domain>"));
    assert!(RELEASE_NETWORK.contains("<domain includeSubdomains=\"false\">localhost</domain>"));
    assert!(
        !RELEASE_NETWORK.contains("10.0.2.2"),
        "the emulator host must stay debug-only"
    );
}

#[test]
fn debug_network_policy_still_reaches_the_emulator_host() {
    assert!(DEBUG_NETWORK.contains("10.0.2.2"));
    assert!(DEBUG_NETWORK.contains("127.0.0.1"));
}

#[test]
fn only_the_launcher_activity_is_exported() {
    assert_eq!(
        MANIFEST.matches("android:exported=\"true\"").count(),
        1,
        "only PinpointActivity is an entry point"
    );
    assert!(MANIFEST.contains(r#"android:name=".PinpointLocationService""#));
    assert!(MANIFEST.contains(r#"android:name=".LocationWarmupReceiver""#));
    assert_eq!(
        MANIFEST.matches("android:exported=\"false\"").count(),
        2,
        "service and receiver must stay internal"
    );
}
