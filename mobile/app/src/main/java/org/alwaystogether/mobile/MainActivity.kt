package org.alwaystogether.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import org.alwaystogether.core.map.MapLibreInit
import org.alwaystogether.feature.auth.AuthViewModel
import org.alwaystogether.feature.auth.LoginScreen
import org.alwaystogether.feature.friends.FriendsScreen
import org.alwaystogether.feature.friends.FriendsViewModel
import org.alwaystogether.feature.map.MapScreen
import org.alwaystogether.feature.map.MapViewModel
import org.alwaystogether.feature.sharing.SharingScreen
import org.alwaystogether.feature.sharing.SharingViewModel
import org.alwaystogether.mobile.di.Container

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val container = (application as MobileApp).container
        MapLibreInit.initialize(applicationContext)

        setContent {
            MaterialTheme {
                AppNav(container = container)
            }
        }
    }
}

private data class Destination(val route: String, val label: String, val icon: ImageVector)

// Icons.Filled.{Place,Person,Share} are in material-icons-core; the extended
// set (which contains Map/Group) is a multi-MB dependency — avoided on purpose.
private val destinations = listOf(
    Destination("map", "Map", Icons.Filled.Place),
    Destination("friends", "Friends", Icons.Filled.Person),
    Destination("sharing", "Sharing", Icons.Filled.Share),
)

@Composable
fun AppNav(container: Container) {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = backStackEntry?.destination?.route

    Scaffold(
        bottomBar = {
            if (currentRoute != null && currentRoute != "login" && currentRoute != "splash") {
                NavigationBar {
                    destinations.forEach { destination ->
                        NavigationBarItem(
                            selected = currentRoute == destination.route,
                            onClick = {
                                navController.navigate(destination.route) {
                                    popUpTo(navController.graph.findStartDestination().id) {
                                        saveState = true
                                    }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                            icon = { Icon(destination.icon, contentDescription = destination.label) },
                            label = { Text(destination.label) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = "splash",
            modifier = Modifier.padding(padding),
        ) {
            composable("splash") {
                // One-shot session restore at app start: instant login when a
                // valid encrypted session exists, login screen otherwise.
                LaunchedEffect(Unit) {
                    val restored = container.session.restore()
                    navController.navigate(if (restored) "map" else "login") {
                        popUpTo("splash") { inclusive = true }
                    }
                }
                Text("")
            }
            composable("login") {
                val viewModel: AuthViewModel = viewModel(
                    factory = AuthViewModel.factory(container.authRepository),
                )
                LoginScreen(
                    viewModel = viewModel,
                    onLoggedIn = {
                        navController.navigate("map") {
                            popUpTo("login") { inclusive = true }
                        }
                    },
                )
            }
            composable("map") {
                val viewModel: MapViewModel = viewModel(
                    factory = MapViewModel.factory(container.offlineMaps),
                )
                MapScreen(viewModel = viewModel)
            }
            composable("friends") {
                val viewModel: FriendsViewModel = viewModel(
                    factory = FriendsViewModel.factory(
                        container.friendshipRepository,
                        container.session.userId(),
                    ),
                )
                FriendsScreen(viewModel = viewModel)
            }
            composable("sharing") {
                val viewModel: SharingViewModel = viewModel(
                    factory = SharingViewModel.factory(
                        container.locationRepository,
                        container.friendshipRepository,
                        container.session.userId(),
                    ),
                )
                SharingScreen(viewModel = viewModel)
            }
        }
    }
}
