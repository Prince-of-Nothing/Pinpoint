// Application wiring: router construction and route registration.
package handlers

import (
	"log"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/go-chi/chi/v5"
	chimw "github.com/go-chi/chi/v5/middleware"

	"github.com/always-together/backend/internal/config"
	"github.com/always-together/backend/internal/store"
)

// App bundles the dependencies shared by all HTTP handlers.
type App struct {
	Settings    *config.Settings
	Store       store.Repository
	RateLimiter *RateLimiter

	// Per-email lockout expiry (Redis in production).
	lockMu   sync.Mutex
	lockouts map[string]time.Time
}

// NewApp constructs an application with the store selected by DB_DRIVER
// (memory by default; postgres with graceful fallback to memory).
func NewApp() *App {
	settings := config.Get()
	repo, err := store.Open(settings.DBDriver, settings.DatabaseURL)
	if err != nil {
		log.Fatalf("store: %v", err) // unknown driver: fail closed on typos
	}
	return &App{
		Settings:    settings,
		Store:       repo,
		RateLimiter: NewRateLimiter(),
		lockouts:    make(map[string]time.Time),
	}
}

// NewAppWithStore constructs an application around a specific repository
// (tests, future DI wiring).
func NewAppWithStore(settings *config.Settings, repo store.Repository) *App {
	return &App{
		Settings:    settings,
		Store:       repo,
		RateLimiter: NewRateLimiter(),
		lockouts:    make(map[string]time.Time),
	}
}

// SeedIfEmpty populates demo data when SEED_DEMO_DATA=true and the store is
// empty. Returns true if accounts were created.
func (a *App) SeedIfEmpty() bool {
	if !a.Settings.SeedDemo {
		return false
	}
	return a.Store.SeedDemoData()
}

// Router builds the chi router with middleware and all API routes mounted,
// under the /api/v1 prefix.
func (a *App) Router() http.Handler {
	r := chi.NewRouter()

	// Server-level middleware (request-id/logging/recovery equivalents).
	r.Use(chimw.RequestID)
	r.Use(chimw.RealIP)
	r.Use(chimw.Recoverer)
	r.Use(corsMiddleware)

	// Root + health endpoints.
	r.Get("/", func(w http.ResponseWriter, req *http.Request) {
		WriteJSON(w, http.StatusOK, map[string]string{
			"name":    a.Settings.AppName,
			"version": Version,
			"health":  "/health",
		})
	})
	r.Get("/health", func(w http.ResponseWriter, req *http.Request) {
		WriteJSON(w, http.StatusOK, map[string]string{
			"status":  "healthy",
			"version": Version,
		})
	})

	// /api/v1/auth
	r.Route("/api/v1/auth", func(r chi.Router) {
		r.Post("/login", a.handleLogin)
		r.Post("/logout", a.handleLogout)
		r.With(a.requireAuth).Post("/password/change", a.handleChangePassword)
		r.With(a.requireAuth).Post("/password/force-change", a.handleForceChangePassword)
		r.With(a.requireAuth).Get("/me", a.handleMe)
		// Session bootstrap for cookie clients (SPA / weba / webk): refreshes
		// the idle timer and re-issues the double-submit CSRF token in the
		// X-CSRF-Token response header. 401 when there is no valid session.
		r.With(a.requireAuth).Post("/session", a.handleSessionRefresh)
	})

	// /api/v1/users
	r.Route("/api/v1/users", func(r chi.Router) {
		r.With(a.requireAdmin).Post("/", a.handleCreateUser)
		r.With(a.requireAuth).Get("/", a.handleListUsers)
		r.With(a.requireAuth).Get("/{userID}", a.handleGetUser)
		r.With(a.requireAuth).Patch("/{userID}", a.handleUpdateUser)
	})

	// /api/v1/friendships
	r.Route("/api/v1/friendships", func(r chi.Router) {
		r.With(a.requireAuth).Post("/", a.handleCreateFriendship)
		r.With(a.requireAuth).Get("/", a.handleListFriendships)
		r.With(a.requireAuth).Post("/{friendshipID}/respond", a.handleRespondFriendship)
	})

	// /api/v1/locations
	r.Route("/api/v1/locations", func(r chi.Router) {
		r.With(a.requireAuth).Post("/shares", a.handleCreateShare)
		r.With(a.requireAuth).Get("/shares", a.handleListShares)
		r.With(a.requireAuth).Patch("/shares/{shareID}", a.handleUpdateShare)
		r.With(a.requireAuth).Post("/me", a.handleUpdateMyLocation)
		r.With(a.requireAuth).Get("/friends", a.handleListFriendLocations)
	})

	// Consistent 404/405 envelopes instead of plain-text defaults.
	r.NotFound(func(w http.ResponseWriter, req *http.Request) {
		if h := a.staticFileServer(); h != nil && !strings.HasPrefix(req.URL.Path, "/api/") {
			h.ServeHTTP(w, req) // bundled SPA (WEB_ROOT env var)
			return
		}
		WriteError(w, req, errf(http.StatusNotFound, "NOT_FOUND", "Resource not found."))
	})
	r.MethodNotAllowed(func(w http.ResponseWriter, req *http.Request) {
		WriteError(w, req, errf(http.StatusMethodNotAllowed, "METHOD_NOT_ALLOWED",
			"Method not allowed."))
	})

	return r
}
