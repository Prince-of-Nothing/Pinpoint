// Driver selection: DB_DRIVER=memory|postgres (T01).
//
// Open returns the configured Repository. When the driver is postgres but the
// database is unreachable, it falls back to the in-memory store with a logged
// warning so local development and demos never hard-fail on infrastructure.
package store

import (
	"fmt"
	"log"
	"strings"
)

// DriverMemory and DriverPostgres are the supported DB_DRIVER values.
const (
	DriverMemory   = "memory"
	DriverPostgres = "postgres"
)

// Open constructs the Repository for the given driver. Unknown drivers are an
// error (fail closed on typos); an unreachable postgres falls back to memory.
func Open(driver, databaseURL string) (Repository, error) {
	switch strings.ToLower(strings.TrimSpace(driver)) {
	case "", DriverMemory:
		return New(), nil
	case DriverPostgres, "postgresql":
		pg, err := OpenPostgres(databaseURL)
		if err != nil {
			log.Printf("store: postgres unavailable (%v); falling back to in-memory store", err)
			return New(), nil
		}
		return pg, nil
	default:
		return nil, fmt.Errorf("store: unknown DB_DRIVER %q (want %q or %q)",
			driver, DriverMemory, DriverPostgres)
	}
}
