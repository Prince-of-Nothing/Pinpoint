// Package config loads application configuration from environment variables.
package config

import (
	"log"
	"os"
	"strconv"
	"strings"
	"sync"
)

// Settings holds all tunable application configuration. Values are loaded
// from environment variables (e.g. DATABASE_URL, SECRET_KEY) with defaults
// matching .env.example.
type Settings struct {
	// Database
	DatabaseURL string

	// Driver selects the persistence backend: memory (default, per-process)
	// or postgres. Postgres falls back to memory with a warning when the
	// database is unreachable (local development).
	DBDriver string

	// Redis
	RedisURL string

	// Security
	SecretKey              string
	Algorithm              string
	AccessTokenExpireMins  int
	RefreshTokenExpireDays int

	// Session settings
	SessionCookieName      string
	SessionMaxAgeSeconds   int
	SessionIdleTimeoutSecs int

	// Rate limiting
	RateLimitRequests      int
	RateLimitWindowSeconds int

	// Account lockout
	LockoutThreshold    int
	LockoutDurationMins int

	// Cookies: set COOKIE_INSECURE=true for local development over plain HTTP
	// (omits the Secure flag so session cookies work on http://localhost).
	CookieInsecure bool

	// SeedDemoData populates a fresh in-memory store with demo accounts.
	SeedDemo bool

	// CORS
	AllowedOrigins []string

	// Application
	AppName string
	Debug   bool
}

var (
	once     sync.Once
	instance *Settings
)

// Get returns a cached Settings instance, loading environment variables on
// first use.
func Get() *Settings {
	once.Do(func() {
		instance = load()
	})
	return instance
}

func envStr(key, def string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return def
}

func envInt(key string, def int) int {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		n, err := strconv.Atoi(v)
		if err != nil {
			log.Printf("config: invalid integer for %s=%q, using default %d", key, v, def)
			return def
		}
		return n
	}
	return def
}

func envBool(key string, def bool) bool {
	v := strings.TrimSpace(os.Getenv(key))
	if v == "" {
		return def
	}
	b, err := strconv.ParseBool(strings.ToLower(v))
	if err != nil {
		return def
	}
	return b
}

func load() *Settings {
	origins := envStr("ALLOWED_ORIGINS", "http://localhost:3000,http://localhost:8080")
	parts := strings.Split(origins, ",")
	allowed := make([]string, 0, len(parts))
	for _, p := range parts {
		if trimmed := strings.TrimSpace(p); trimmed != "" {
			allowed = append(allowed, trimmed)
		}
	}

	return &Settings{
		DatabaseURL: envStr("DATABASE_URL",
			"postgresql://postgres:postgres@localhost:5432/always_together"),
		DBDriver:    envStr("DB_DRIVER", "memory"),
		RedisURL:    envStr("REDIS_URL", "redis://localhost:6379/0"),

		SecretKey: envStr("SECRET_KEY",
			"change-this-secret-key-in-production-min-32-characters-long"),
		Algorithm:              envStr("ALGORITHM", "HS256"),
		AccessTokenExpireMins:  envInt("ACCESS_TOKEN_EXPIRE_MINUTES", 30),
		RefreshTokenExpireDays: envInt("REFRESH_TOKEN_EXPIRE_DAYS", 7),

		SessionCookieName:      envStr("SESSION_COOKIE_NAME", "session_id"),
		SessionMaxAgeSeconds:   envInt("SESSION_MAX_AGE_SECONDS", 10800),
		SessionIdleTimeoutSecs: envInt("SESSION_IDLE_TIMEOUT_SECONDS", 1800),

		RateLimitRequests:      envInt("RATE_LIMIT_REQUESTS", 10),
		RateLimitWindowSeconds: envInt("RATE_LIMIT_WINDOW_SECONDS", 60),

		LockoutThreshold:    envInt("LOCKOUT_THRESHOLD", 5),
		LockoutDurationMins: envInt("LOCKOUT_DURATION_MINUTES", 15),

		AllowedOrigins: allowed,

		CookieInsecure: envBool("COOKIE_INSECURE", false),
		SeedDemo:       envBool("SEED_DEMO_DATA", false),

		AppName: envStr("APP_NAME", "Always Together API"),
		Debug:   envBool("DEBUG", true),
	}
}
