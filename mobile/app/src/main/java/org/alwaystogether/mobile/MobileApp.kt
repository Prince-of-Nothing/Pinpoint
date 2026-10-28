package org.alwaystogether.mobile

import android.app.Application
import org.alwaystogether.mobile.di.Container

/**
 * Application entry point and manual-DI composition root.
 *
 * No Hilt/Kapt: the container is constructed explicitly so every dependency
 * is visible in one file and swappable per build flavor or test. Modules stay
 * free of DI frameworks and only ever see interfaces from :core:domain.
 */
class MobileApp : Application() {

    lateinit var container: Container
        private set

    override fun onCreate() {
        super.onCreate()
        container = Container(this)
    }
}
