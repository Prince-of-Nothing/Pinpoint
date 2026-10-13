// Middleware: CORS, authentication (JWT bearer + server-side sessions), and
// per-IP login rate limiting.
package handlers

import (
	"context"
	"net/http"
	"strconv"
	"sync"
	"time"

	"github.com/always-together/backend/internal/config"
	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

type ctxKey string

const userKey ctxKey = "authenticated_user"

// userFromContext returns the user injected by RequireAuth, if any.
func userFromContext(ctx context.Context) *models.User {
	u, _ := ctx.Value(userKey).(*models.User)
	return u
}

// corsMiddleware allows configured origins with credentials, standard
// methods, all headers and exposes the X-CSRF-Token response header.
func corsMiddleware(next http.Handler) http.Handler {
	s := config.Get()
	allowed := map[string]bool{}
	for _, o := range s.AllowedOrigins {
		allowed[o] = true
	}

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		origin := r.Header.Get("Origin")
		if origin != "" && allowed[origin] {
			h := w.Header()
			h.Set("Access-Control-Allow-Origin", origin)
			h.Set("Access-Control-Allow-Credentials", "true")
			h.Add("Vary", "Origin")
			h.Set("Access-Control-Expose-Headers", "X-CSRF-Token")
			h.Set("Access-Control-Max-Age", "600")
		}

		if r.Method == http.MethodOptions {
			h := w.Header()
			h.Set("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
			h.Set("Access-Control-Allow-Headers", r.Header.Get("Access-Control-Request-Headers"))
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

// csrfHeader is the header carrying the double-submit CSRF token. It is
// exposed via CORS (Access-Control-Expose-Headers: X-CSRF-Token) so the SPA
// can read it from the login response.
const csrfHeader = "X-CSRF-Token"

// requireAuth authenticates a request via the Bearer JWT or a valid
// session cookie; on success the user is placed on the request context.
//
// When no Bearer token is present but a valid session cookie exists, the
// request is authenticated through the server-side session instead. In that
// case state-changing methods (POST/PUT/PATCH/DELETE) must also present a
// matching X-CSRF-Token header (double-submit cookie pattern), because the
// session cookie is sent automatically by browsers.
func (a *App) requireAuth(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var user *models.User

		if token := bearerToken(r); token != "" {
			claims := security.DecodeToken(token)
			if claims == nil {
				WriteError(w, r, errf(http.StatusUnauthorized, "INVALID_TOKEN",
					"Invalid or expired token."))
				return
			}
			id, err := strconv.ParseInt(claims.Subject, 10, 64)
			if err != nil {
				WriteError(w, r, errf(http.StatusUnauthorized, "INVALID_TOKEN",
					"Invalid or expired token."))
				return
			}
			u, err := a.Store.GetUserByID(id)
			if err != nil {
				WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND", "User not found."))
				return
			}
			user = u
		} else if c, err := r.Cookie(config.Get().SessionCookieName); err == nil && c.Value != "" {
			sess, err := a.Store.GetSession(c.Value)
			if err != nil {
				WriteError(w, r, errf(http.StatusUnauthorized, "INVALID_SESSION",
					"Not authenticated."))
				return
			}
			u, err := a.Store.GetUserByID(sess.UserID)
			if err != nil || !sess.IsValid(u, time.Now().UTC()) {
				WriteError(w, r, errf(http.StatusUnauthorized, "INVALID_SESSION",
					"Not authenticated."))
				return
			}
			// Double-submit CSRF check for unsafe methods.
			switch r.Method {
			case http.MethodPost, http.MethodPut, http.MethodPatch, http.MethodDelete:
				if !security.VerifyCSRFToken(r.Header.Get(csrfHeader), sess.CSRFToken) {
					WriteError(w, r, errf(http.StatusForbidden, "CSRF_FAILED",
						"Missing or invalid CSRF token."))
					return
				}
			}
			a.Store.TouchSession(sess.SessionToken)
			// Re-mirror the CSRF token cookie so reloads keep working even
			// if the browser dropped it (Max-Age/session-cookie edge cases).
			http.SetCookie(w, csrfCookie(sess.CSRFToken, config.Get().SessionMaxAgeSeconds))
			user = u
		} else {
			WriteError(w, r, errf(http.StatusUnauthorized, "INVALID_TOKEN",
				"Not authenticated."))
			return
		}

		if !user.IsActive || user.IsLocked {
			WriteError(w, r, errf(http.StatusUnauthorized, "ACCOUNT_INACTIVE",
				"Account is inactive or locked."))
			return
		}
		next.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), userKey, user)))
	})
}

// sessionCookie builds the secure session cookie set on successful login
// (httponly, secure, SameSite=Lax, path=/).
func sessionCookie(value string, maxAge int) *http.Cookie {
	return &http.Cookie{
		Name:     config.Get().SessionCookieName,
		Value:    value,
		Path:     "/",
		MaxAge:   maxAge,
		HttpOnly: true,
		Secure:   !config.Get().CookieInsecure,
		SameSite: http.SameSiteLaxMode,
	}
}

// csrfCookie builds the non-httponly companion cookie that mirrors the
// session's double-submit CSRF token, so cookie clients (SPA, weba, webk)
// survive page reloads: JS reads it and echoes it back in X-CSRF-Token.
func csrfCookie(value string, maxAge int) *http.Cookie {
	return &http.Cookie{
		Name:     "csrf_token",
		Value:    value,
		Path:     "/",
		MaxAge:   maxAge,
		HttpOnly: false,
		Secure:   !config.Get().CookieInsecure,
		SameSite: http.SameSiteLaxMode,
	}
}

// clearCookie builds the expired replacement cookie used on logout.
func clearCookie() *http.Cookie {
	return &http.Cookie{
		Name:     config.Get().SessionCookieName,
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		Secure:   !config.Get().CookieInsecure,
		SameSite: http.SameSiteLaxMode,
	}
}

// RateLimiter is a fixed-window per-key limiter approximating the Redis-based
// scheme described by RATE_LIMIT_REQUESTS / RATE_LIMIT_WINDOW_SECONDS.
type RateLimiter struct {
	mu     sync.Mutex
	hits   map[string][]time.Time
	limit  int
	window time.Duration
}

// NewRateLimiter creates a limiter from the application settings.
func NewRateLimiter() *RateLimiter {
	s := config.Get()
	return &RateLimiter{
		hits:   make(map[string][]time.Time),
		limit:  s.RateLimitRequests,
		window: time.Duration(s.RateLimitWindowSeconds) * time.Second,
	}
}

// Allow reports whether a hit for key is permitted under the limit.
func (rl *RateLimiter) Allow(key string) bool {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	now := time.Now()
	cutoff := now.Add(-rl.window)

	kept := rl.hits[key][:0]
	for _, t := range rl.hits[key] {
		if t.After(cutoff) {
			kept = append(kept, t)
		}
	}
	if len(kept) >= rl.limit {
		rl.hits[key] = kept
		return false
	}
	rl.hits[key] = append(kept, now)
	return true
}

func rateLimitKey(ip string) string  { return "rate_limit:login:" + ip }
func lockoutKey(email string) string { return "lockout:" + email }
