package com.pinpoint.desktop;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Receives the one-shot location warm-up updates requested from Rust.
 *
 * <p>{@code LocationManager.requestLocationUpdates} with a PendingIntent needs
 * a receiver for the intent to resolve, otherwise the system drops every fix
 * and the app only ever sees a stale last-known location.
 */
public class LocationWarmupReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        Log.d("PinpointLocation", "warm-up location update received");
    }
}
