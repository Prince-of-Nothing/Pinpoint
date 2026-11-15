package com.pinpoint.desktop;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.Build;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;

/**
 * User-started foreground service that publishes the current location on a
 * chosen interval while Pinpoint is in the background.
 *
 * <p>Only started from the explicit Profile action. The persistent
 * notification names the interval and the app; stopping it removes both the
 * updates and the notification. No location history is kept on-device.
 */
public class PinpointLocationService extends Service implements LocationListener {
    public static final String ACTION_START = "com.pinpoint.desktop.START_LOCATION";
    public static final String ACTION_STOP = "com.pinpoint.desktop.STOP_LOCATION";
    private static final String CHANNEL_ID = "pinpoint_location";
    private static final int NOTIFICATION_ID = 42;

    private static volatile boolean running = false;

    static {
        System.loadLibrary("pinpoint_desktop");
    }

    private static native void nativePublishLocation(double latitude, double longitude, double accuracy);

    private LocationManager locationManager;
    private int intervalSeconds = 60;

    public static boolean isRunning() {
        return running;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        createChannel();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        if (checkSelfPermission(android.Manifest.permission.ACCESS_FINE_LOCATION)
                != PackageManager.PERMISSION_GRANTED) {
            Log.w("PinpointService", "foreground service started without location permission");
            stopSelf();
            return START_NOT_STICKY;
        }
        if (intent != null) {
            intervalSeconds = Math.max(15, intent.getIntExtra("interval_secs", 60));
        }
        startForeground(NOTIFICATION_ID, buildNotification());
        startUpdates();
        running = true;
        return START_STICKY;
    }

    @Override
    public void onDestroy() {
        running = false;
        if (locationManager != null) {
            try {
                locationManager.removeUpdates(this);
            } catch (Throwable error) {
                Log.w("PinpointService", "removeUpdates failed", error);
            }
            locationManager = null;
        }
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public void onLocationChanged(Location location) {
        if (location == null) {
            return;
        }
        try {
            nativePublishLocation(
                location.getLatitude(),
                location.getLongitude(),
                location.getAccuracy()
            );
        } catch (Throwable error) {
            Log.w("PinpointService", "native publish failed", error);
        }
    }

    private void startUpdates() {
        if (locationManager != null) {
            return;
        }
        locationManager = (LocationManager) getSystemService(Context.LOCATION_SERVICE);
        if (locationManager == null) {
            Log.w("PinpointService", "no location manager");
            stopSelf();
            return;
        }
        long minTimeMs = intervalSeconds * 1000L;
        for (String provider : new String[] {
            LocationManager.GPS_PROVIDER,
            LocationManager.NETWORK_PROVIDER,
            LocationManager.PASSIVE_PROVIDER,
        }) {
            try {
                locationManager.requestLocationUpdates(
                    provider,
                    minTimeMs,
                    0f,
                    this,
                    Looper.getMainLooper()
                );
            } catch (Throwable error) {
                Log.w("PinpointService", "provider " + provider + " unavailable", error);
            }
        }
    }

    private void createChannel() {
        NotificationManager manager =
            (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager == null) {
            return;
        }
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID,
            "Location sharing",
            NotificationManager.IMPORTANCE_LOW
        );
        channel.setDescription("Shows when Pinpoint is publishing your location in the background.");
        manager.createNotificationChannel(channel);
    }

    private Notification buildNotification() {
        return new Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("Pinpoint is sharing your location")
            .setContentText("Every " + intervalSeconds + " seconds. Stop it in Profile & settings.")
            .setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setOngoing(true)
            .build();
    }
}
