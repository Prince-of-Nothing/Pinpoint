// Repository interfaces extracted from the concrete in-memory Store (T01).
//
// Handlers depend on these narrow interfaces instead of the concrete store,
// so the PostgreSQL implementation can be swapped in via DB_DRIVER without
// touching HTTP code. Each sub-interface covers one entity family; Repository
// composes them for wiring convenience.
package store

import (
	"time"

	"github.com/always-together/backend/internal/models"
)

// UserStore persists accounts and their security state.
type UserStore interface {
	CreateUser(u *models.User) error
	GetUserByID(id int64) (*models.User, error)
	GetUserByEmail(normEmail string) (*models.User, error)
	ListUsers() []models.User
	UpdateUser(u *models.User) error
	SetUserActive(id int64, active bool) error
	UnlockUser(id int64) error
}

// SessionStore persists server-side sessions (login tracking, CSRF, idle
// timeout, revocation).
type SessionStore interface {
	CreateSession(sess *models.Session) error
	GetSession(token string) (*models.Session, error)
	RevokeSession(token string)
	RevokeAllSessionsForUser(userID int64)
	TouchSession(token string)
}

// FriendshipStore persists friend requests and their lifecycle.
type FriendshipStore interface {
	CreateFriendship(f *models.Friendship) error
	GetFriendship(id int64) (*models.Friendship, error)
	UpdateFriendship(f *models.Friendship) error
	ListFriendshipsForUser(userID int64) []models.Friendship
}

// ShareStore persists directional location-sharing permissions.
type ShareStore interface {
	CreateLocationShare(ls *models.LocationShare) error
	GetLocationShare(id int64) (*models.LocationShare, error)
	UpdateLocationShare(ls *models.LocationShare) error
	ListLocationSharesByOwner(ownerID int64) []models.LocationShare
	ListActiveSharesVisibleTo(ownerID, viewerID int64) []models.LocationShare
}

// AttemptStore records authentication attempts for auditing and lockout.
type AttemptStore interface {
	RecordLoginAttempt(email, ip string, success bool)
	RecentFailedAttempts(email string, window time.Duration) int
}

// Seeder populates demo data on an empty store (local development only).
// Returns true when accounts were created.
type Seeder interface {
	SeedDemoData() bool
}

// Repository is the full persistence surface consumed by the application.
type Repository interface {
	UserStore
	SessionStore
	FriendshipStore
	ShareStore
	AttemptStore
	Seeder
}

// Compile-time checks: both drivers must satisfy the full repository.
var (
	_ Repository = (*Store)(nil)
)
