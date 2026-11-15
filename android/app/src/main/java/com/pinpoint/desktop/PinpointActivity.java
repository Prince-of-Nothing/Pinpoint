package com.pinpoint.desktop;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Bundle;

/**
 * Thin Java bridge around {@link NativeActivity}.
 *
 * <p>Android delivers shares, deep links and notification taps to
 * {@code onNewIntent} while the activity is alive. Plain {@code NativeActivity}
 * does not call {@code setIntent}, so the Rust side would keep seeing the cold
 * start intent. This subclass stores the newest intent and exposes repeat
 * shares to Rust through {@link #consumeShare()}.
 *
 * <p>It also hosts the biometric prompt and the foreground location service
 * (ADR-0017). No account data is stored here.
 */
public class PinpointActivity extends NativeActivity {
    static {
        // NativeActivity loads the library for android_main; registering it with
        // the class loader too makes JNI native methods resolvable.
        System.loadLibrary("pinpoint_desktop");
    }

    private static native void nativeWakeUi();

    private final Object shareLock = new Object();
    private String pendingShare;
    private static volatile int biometricState = 0;
    private android.os.CancellationSignal biometricCancel;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        captureShare(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        captureShare(intent);
        nativeWakeUi();
    }

    /** True when the device advertises biometric hardware and is API 28+. */
    public boolean isBiometricSupported() {
        if (android.os.Build.VERSION.SDK_INT < 28) {
            return false;
        }
        try {
            android.content.pm.PackageManager pm = getPackageManager();
            return pm.hasSystemFeature(android.content.pm.PackageManager.FEATURE_FINGERPRINT)
                || pm.hasSystemFeature(android.content.pm.PackageManager.FEATURE_FACE)
                || pm.hasSystemFeature(android.content.pm.PackageManager.FEATURE_IRIS);
        } catch (Throwable error) {
            android.util.Log.w("PinpointBiometric", "support check failed", error);
            return false;
        }
    }

    /**
     * Start the platform biometric prompt. Returns 1 when a prompt was
     * requested, 0 when biometrics are unavailable. The result is polled via
     * {@link #pollBiometric()} from Rust; the callback only stores state and
     * wakes the UI.
     */
    public int startBiometric() {
        if (!isBiometricSupported()) {
            biometricState = 4;
            return 0;
        }
        biometricState = 1;
        runOnUiThread(() -> {
            try {
                android.os.CancellationSignal cancel = new android.os.CancellationSignal();
                biometricCancel = cancel;
                java.util.concurrent.Executor executor = getMainExecutor();
                android.hardware.biometrics.BiometricPrompt prompt =
                    new android.hardware.biometrics.BiometricPrompt.Builder(this)
                        .setTitle("Unlock Pinpoint")
                        .setSubtitle("Use your fingerprint or face")
                        .setNegativeButton("Use PIN", executor, (dialog, which) -> {
                            biometricState = 3;
                            nativeWakeUi();
                        })
                        .build();
                prompt.authenticate(
                    cancel,
                    executor,
                    new android.hardware.biometrics.BiometricPrompt.AuthenticationCallback() {
                        @Override
                        public void onAuthenticationSucceeded(
                            android.hardware.biometrics.BiometricPrompt.AuthenticationResult result
                        ) {
                            biometricState = 2;
                            nativeWakeUi();
                        }

                        @Override
                        public void onAuthenticationError(int errorCode, CharSequence errString) {
                            biometricState = 3;
                            nativeWakeUi();
                        }
                    }
                );
            } catch (Throwable error) {
                android.util.Log.w("PinpointBiometric", "prompt failed", error);
                biometricState = 4;
                nativeWakeUi();
            }
        });
        return 1;
    }

    /** 0 idle, 1 pending, 2 success, 3 failed/cancelled, 4 unavailable. */
    public int pollBiometric() {
        return biometricState;
    }

    public void cancelBiometric() {
        biometricState = 3;
        if (biometricCancel != null) {
            biometricCancel.cancel();
            biometricCancel = null;
        }
    }

    /** Start the foreground service. Returns 1 on success, 0 on failure. */
    public int startLocationService(int intervalSeconds) {
        try {
            Intent intent = new Intent(this, PinpointLocationService.class)
                .setAction(PinpointLocationService.ACTION_START)
                .putExtra("interval_secs", intervalSeconds);
            if (android.os.Build.VERSION.SDK_INT >= 26) {
                startForegroundService(intent);
            } else {
                startService(intent);
            }
            return 1;
        } catch (Throwable error) {
            android.util.Log.w("PinpointService", "start failed", error);
            return 0;
        }
    }

    public int stopLocationService() {
        try {
            stopService(new Intent(this, PinpointLocationService.class));
            return 1;
        } catch (Throwable error) {
            android.util.Log.w("PinpointService", "stop failed", error);
            return 0;
        }
    }

    public int isLocationServiceRunning() {
        return PinpointLocationService.isRunning() ? 1 : 0;
    }

    /** Called from Rust once per frame; returns and clears the pending share. */
    public String consumeShare() {
        synchronized (shareLock) {
            String value = pendingShare;
            pendingShare = null;
            if (value != null) {
                android.util.Log.i("PinpointShare", "consumed a shared text intent");
            }
            return value;
        }
    }

    private void captureShare(Intent intent) {
        if (intent == null || !Intent.ACTION_SEND.equals(intent.getAction())) {
            return;
        }
        CharSequence text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT);
        if (text == null) {
            text = intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT);
        }
        if (text == null) {
            return;
        }
        String value = text.toString().trim();
        if (value.isEmpty()) {
            return;
        }
        synchronized (shareLock) {
            pendingShare = value;
        }
        android.util.Log.i("PinpointShare", "captured a shared text intent");
    }
}
