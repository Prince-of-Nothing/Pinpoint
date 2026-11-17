package com.pinpoint.desktop;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.NativeActivity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.GeolocationPermissions;
import android.webkit.JavascriptInterface;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.Toast;

/**
 * Thin Java bridge around {@link NativeActivity}.
 *
 * <p>Android delivers shares, deep links and notification taps to
 * {@code onNewIntent} while the activity is alive. Plain {@code NativeActivity}
 * does not call {@code setIntent}, so the Rust side would keep seeing the cold
 * start intent. This subclass stores the newest intent and exposes repeat
 * shares to Rust through {@link #consumeShare()}.
 *
 * <p>It also hosts the biometric prompt, the foreground location service and
 * the full-screen MapLibre WebView (ADR-0017). No account data is stored here;
 * the WebView only loads the loopback origin chosen by the Rust map server and
 * never receives session or CSRF material.
 */
public class PinpointActivity extends NativeActivity {
    static {
        // NativeActivity loads the library for android_main; registering it with
        // the class loader too makes JNI native methods resolvable.
        System.loadLibrary("pinpoint_desktop");
    }

    private static native void nativeWakeUi();
    private static native void nativeMapClick(double latitude, double longitude);
    private static native void nativeMapWebViewClosed();

    private static final String MAP_BRIDGE_NAME = "PinpointBridge";
    private static final int REQ_LOCATION = 1001;
    private static final int REQ_NOTIFICATIONS = 1002;

    /**
     * Binds the preview page's map handle to the Java bridge. The contract is
     * generic: any page can call {@code PinpointBridge.onMapClick(lat, lon)} or
     * {@code PinpointBridge.onShareRequest(lat, lon, label)} itself. This shim
     * only makes the unchanged web/map preview work without page edits.
     */
    private static final String MAP_BRIDGE_BOOTSTRAP =
        "(function(){try{"
            + "if(window.__pinpointBridgeBound){return \"bound\";}"
            + "var preview=window.__pinpointPreview;"
            + "var handle=preview&&preview.handle;"
            + "if(handle&&typeof handle.onClick===\"function\"){"
            + "handle.onClick(function(point){"
            + "try{if(window.PinpointBridge&&PinpointBridge.onMapClick){"
            + "PinpointBridge.onMapClick(point.lat,point.lon);}}catch(error){}});"
            + "window.__pinpointBridgeBound=true;return \"preview\";"
            + "}return \"no-handle\";"
            + "}catch(error){return \"error\";}})()";

    private final Object shareLock = new Object();
    private String pendingShare;
    private static volatile int biometricState = 0;
    private android.os.CancellationSignal biometricCancel;

    private FrameLayout mapOverlay;
    private WebView mapWebView;
    private String mapHost;
    private int mapPort = -1;
    private String pendingGeoOrigin;
    private GeolocationPermissions.Callback pendingGeoCallback;
    private int pendingLocationInterval = -1;

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

    @Override
    protected void onResume() {
        super.onResume();
        if (mapWebView != null && mapOverlay != null
            && mapOverlay.getVisibility() == View.VISIBLE) {
            mapWebView.onResume();
        }
    }

    @Override
    protected void onPause() {
        if (mapWebView != null && mapOverlay != null
            && mapOverlay.getVisibility() == View.VISIBLE) {
            mapWebView.onPause();
        }
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        if (mapWebView != null) {
            mapWebView.destroy();
            mapWebView = null;
        }
        mapOverlay = null;
        super.onDestroy();
    }

    /** The WebView close control and the system back gesture both land here. */
    @SuppressWarnings("deprecation")
    @Override
    public void onBackPressed() {
        if (mapOverlay != null && mapOverlay.getVisibility() == View.VISIBLE) {
            hideMapWebView();
            return;
        }
        super.onBackPressed();
    }

    /**
     * Show the loopback MapLibre page in a full-screen WebView above the egui
     * surface. Called from Rust; the URL is validated again here so a bad JNI
     * call cannot navigate the WebView off loopback.
     */
    public void showMapWebView(final String url) {
        final Uri parsed = Uri.parse(url);
        if (!"http".equals(parsed.getScheme())
            || !"127.0.0.1".equals(parsed.getHost())
            || parsed.getPort() <= 0
            || parsed.getPort() > 65535) {
            android.util.Log.w("PinpointMap", "refused a non-loopback map URL");
            return;
        }
        runOnUiThread(() -> {
            mapHost = parsed.getHost();
            mapPort = parsed.getPort();
            if (mapOverlay == null) {
                mapOverlay = buildMapOverlay();
                addContentView(
                    mapOverlay,
                    new ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                );
            }
            mapOverlay.setVisibility(View.VISIBLE);
            mapOverlay.bringToFront();
            mapOverlay.requestFocus();
            if ((getApplicationInfo().flags
                    & android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
                WebView.setWebContentsDebuggingEnabled(true);
            }
            mapWebView.onResume();
            mapWebView.loadUrl(url);
        });
    }

    /** Hide the overlay and tell Rust so the map screen button stays in sync. */
    public void hideMapWebView() {
        runOnUiThread(() -> {
            if (mapOverlay == null) {
                return;
            }
            mapOverlay.setVisibility(View.GONE);
            if (mapWebView != null) {
                mapWebView.onPause();
            }
            nativeMapWebViewClosed();
        });
    }

    /**
     * Share a map point as a {@code geo:} link through the system chooser.
     * Used by {@code PinpointBridge.onShareRequest} and by the Rust map screen.
     * Returns 1 when the request was accepted for the UI thread.
     */
    public int shareGeo(final double latitude, final double longitude, final String label) {
        if (!validGeo(latitude, longitude)) {
            return 0;
        }
        runOnUiThread(() -> {
            String suffix = (label == null || label.trim().isEmpty())
                ? ""
                : "?q=" + Uri.encode(label.trim());
            String link = "geo:" + latitude + "," + longitude + suffix;
            Intent send = new Intent(Intent.ACTION_SEND);
            send.setType("text/plain");
            send.putExtra(Intent.EXTRA_TEXT, link);
            try {
                startActivity(Intent.createChooser(send, "Share location"));
            } catch (Throwable error) {
                android.util.Log.w("PinpointMap", "share sheet failed", error);
            }
        });
        return 1;
    }

    private static boolean validGeo(double latitude, double longitude) {
        return Double.isFinite(latitude)
            && Double.isFinite(longitude)
            && latitude >= -90.0
            && latitude <= 90.0
            && longitude >= -180.0
            && longitude <= 180.0;
    }

    private FrameLayout buildMapOverlay() {
        FrameLayout overlay = new FrameLayout(this);
        overlay.setBackgroundColor(Color.rgb(244, 241, 234));

        WebView web = new WebView(this);
        configureMapWebView(web);
        overlay.addView(
            web,
            new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        );

        Button close = new Button(this);
        close.setText("Close map");
        close.setAllCaps(false);
        close.setContentDescription("Close the offline map");
        FrameLayout.LayoutParams closeParams = new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            Gravity.TOP | Gravity.END
        );
        closeParams.setMargins(16, 16, 16, 16);
        close.setOnClickListener(view -> hideMapWebView());
        overlay.addView(close, closeParams);

        mapWebView = web;
        return overlay;
    }

    @SuppressLint("SetJavaScriptEnabled")
    @SuppressWarnings("deprecation")
    private void configureMapWebView(WebView web) {
        WebSettings settings = web.getSettings();
        settings.setJavaScriptEnabled(true);
        // The preview stores its light/dark choice in localStorage.
        settings.setDomStorageEnabled(true);
        // The page is fully local: no file/content origins and no remote nav.
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setAllowFileAccessFromFileURLs(false);
        settings.setAllowUniversalAccessFromFileURLs(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setMediaPlaybackRequiresUserGesture(true);
        settings.setGeolocationEnabled(true);
        settings.setSupportMultipleWindows(false);
        settings.setJavaScriptCanOpenWindowsAutomatically(false);
        settings.setSaveFormData(false);
        settings.setSavePassword(false);
        // OFM sends Cache-Control: max-age=86400, loopback map assets send no-cache, so HTTP caching stays correct without disabling tile caching.
        settings.setCacheMode(WebSettings.LOAD_DEFAULT);
        settings.setSupportZoom(true);
        settings.setBuiltInZoomControls(false);
        web.setBackgroundColor(Color.rgb(244, 241, 234));
        web.setHorizontalScrollBarEnabled(false);
        web.setVerticalScrollBarEnabled(false);
        web.addJavascriptInterface(new MapBridge(), MAP_BRIDGE_NAME);
        web.setWebViewClient(new MapWebViewClient());
        web.setWebChromeClient(new WebChromeClient() {
            @Override
            public void onGeolocationPermissionsShowPrompt(
                    String origin, GeolocationPermissions.Callback callback) {
                if (hasLocationPermission()) {
                    callback.invoke(origin, true, false);
                    return;
                }
                pendingGeoOrigin = origin;
                pendingGeoCallback = callback;
                requestPermissions(
                    new String[] {
                        Manifest.permission.ACCESS_FINE_LOCATION,
                        Manifest.permission.ACCESS_COARSE_LOCATION,
                    },
                    REQ_LOCATION
                );
            }

            @Override
            public boolean onConsoleMessage(android.webkit.ConsoleMessage message) {
                android.util.Log.i(
                    "PinpointMap",
                    message.message() + " @" + message.lineNumber()
                );
                return true;
            }
        });
    }

    /** Only the live loopback origin may navigate the WebView. */
    private boolean isLoopbackMapUrl(Uri uri) {
        return uri != null
            && "http".equals(uri.getScheme())
            && mapHost != null
            && mapHost.equals(uri.getHost())
            && mapPort > 0
            && mapPort == uri.getPort();
    }

    /**
     * Remote navigations are blocked in the WebView. https attribution links
     * (OSM/OpenMapTiles/ODbL) can still leave through the system browser.
     */
    private boolean handleMapNavigation(Uri uri) {
        if (isLoopbackMapUrl(uri)) {
            return false;
        }
        if (uri != null && "https".equalsIgnoreCase(uri.getScheme())) {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, uri));
            } catch (Throwable error) {
                android.util.Log.w("PinpointMap", "external link failed", error);
            }
        } else {
            android.util.Log.w("PinpointMap", "blocked a non-loopback navigation");
        }
        return true;
    }

    private final class MapWebViewClient extends WebViewClient {
        @Override
        public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
            return handleMapNavigation(request.getUrl());
        }

        @Override
        public void onPageFinished(WebView view, String url) {
            super.onPageFinished(view, url);
            if (!isLoopbackMapUrl(Uri.parse(url))) {
                return;
            }
            view.evaluateJavascript(MAP_BRIDGE_BOOTSTRAP, value -> {
                if (value != null && value.contains("no-handle")) {
                    view.postDelayed(() -> {
                        if (mapWebView != null) {
                            mapWebView.evaluateJavascript(MAP_BRIDGE_BOOTSTRAP, null);
                        }
                    }, 500);
                }
            });
        }

        @Override
        public void onReceivedError(
            WebView view,
            WebResourceRequest request,
            WebResourceError error
        ) {
            if (request.isForMainFrame()) {
                android.util.Log.w(
                    "PinpointMap",
                    "main frame failed: " + error.getDescription() + " " + request.getUrl()
                );
            }
        }
    }

    /** The only JavaScript surface exposed to the loopback page. */
    private final class MapBridge {
        @JavascriptInterface
        public void onMapClick(double latitude, double longitude) {
            nativeMapClick(latitude, longitude);
        }

        @JavascriptInterface
        public void onShareRequest(double latitude, double longitude, String label) {
            shareGeo(latitude, longitude, label);
        }
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

    /**
     * Start the foreground service. Returns 1 when the service started or the
     * permission request was issued, 0 on failure.
     *
     * <p>Android only shows the location dialog in response to
     * {@code requestPermissions}; a manifest entry alone never prompts. If the
     * permission is missing the request is issued and the service is started
     * from {@link #onRequestPermissionsResult}.
     */
    public int startLocationService(int intervalSeconds) {
        if (!hasLocationPermission()) {
            pendingLocationInterval = Math.max(15, intervalSeconds);
            requestPermissions(
                new String[] {
                    Manifest.permission.ACCESS_FINE_LOCATION,
                    Manifest.permission.ACCESS_COARSE_LOCATION,
                },
                REQ_LOCATION
            );
            return 1;
        }
        return startForegroundLocation(intervalSeconds);
    }

    private int startForegroundLocation(int intervalSeconds) {
        try {
            if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                    != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(
                    new String[] { Manifest.permission.POST_NOTIFICATIONS },
                    REQ_NOTIFICATIONS
                );
            }
            Intent intent = new Intent(this, PinpointLocationService.class)
                .setAction(PinpointLocationService.ACTION_START)
                .putExtra("interval_secs", intervalSeconds);
            if (Build.VERSION.SDK_INT >= 26) {
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

    private boolean hasLocationPermission() {
        return checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)
                == PackageManager.PERMISSION_GRANTED
            || checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION)
                == PackageManager.PERMISSION_GRANTED;
    }

    @Override
    public void onRequestPermissionsResult(
            int requestCode, String[] permissions, int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode != REQ_LOCATION) {
            return;
        }
        boolean granted = false;
        for (int result : grantResults) {
            if (result == PackageManager.PERMISSION_GRANTED) {
                granted = true;
                break;
            }
        }
        if (pendingGeoCallback != null) {
            pendingGeoCallback.invoke(pendingGeoOrigin, granted, false);
            pendingGeoCallback = null;
            pendingGeoOrigin = null;
        }
        if (granted) {
            if (pendingLocationInterval > 0) {
                int interval = pendingLocationInterval;
                pendingLocationInterval = -1;
                startForegroundLocation(interval);
            }
        } else {
            pendingLocationInterval = -1;
            Toast.makeText(
                this,
                "Location permission is required to show or share your position.",
                Toast.LENGTH_LONG
            ).show();
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
