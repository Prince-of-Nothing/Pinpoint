// Command server starts the Always Together API HTTP server.
//
// Usage:
//
//	go run ./cmd/server            # listens on :8000 by default
//	PORT=9000 go run ./cmd/server
package main

import (
	"context"
	"errors"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/always-together/backend/internal/config"
	"github.com/always-together/backend/internal/handlers"
)

func main() {
	settings := config.Get()

	app := handlers.NewApp()
	if app.SeedIfEmpty() {
		log.Println("seeded demo accounts: alice@example.com/DemoPass123!, bob@example.com/DemoPass456!, admin@example.com/ChangeMe123!")
	}
	srv := &http.Server{
		Addr:              ":" + portFromEnv(),
		Handler:           app.Router(),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Second,
		WriteTimeout:      30 * time.Second,
		IdleTimeout:       time.Duration(settings.SessionIdleTimeoutSecs) * time.Second,
	}

	serverErr := make(chan error, 1)
	go func() {
		log.Printf("%s v%s listening on %s (debug=%t)",
			settings.AppName, handlers.Version, srv.Addr, settings.Debug)
		serverErr <- srv.ListenAndServe()
	}()

	// Graceful shutdown on SIGINT/SIGTERM.
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, syscall.SIGINT, syscall.SIGTERM)

	select {
	case err := <-serverErr:
		if !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("server error: %v", err)
		}
	case sig := <-stop:
		log.Printf("received %s, shutting down...", sig)
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := srv.Shutdown(ctx); err != nil {
			log.Fatalf("graceful shutdown failed: %v", err)
		}
	}
	log.Println("server stopped")
}

func portFromEnv() string {
	if p := os.Getenv("PORT"); p != "" {
		return p
	}
	return "8000"
}
