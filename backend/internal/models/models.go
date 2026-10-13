// Package models defines the domain entities persisted by the store.
// Entities cover users, sessions, friendships, location shares and audits.
package models

import "time"

// UserRole enumerates account roles.
type UserRole string

const (
	RoleResident      UserRole = "resident"
	RoleAdministrator UserRole = "administrator"
)

// FriendshipStatus enumerates friendship request states.
type FriendshipStatus string

const (
	FriendshipPending  FriendshipStatus = "pending"
	FriendshipAccepted FriendshipStatus = "accepted"
	FriendshipRejected FriendshipStatus = "rejected"
	FriendshipBlocked  FriendshipStatus = "blocked"
)

// LocationShareDirection enumerates directional sharing permissions.
type LocationShareDirection string

const (
	ShareToFriend   LocationShareDirection = "to_friend"
	ShareFromFriend LocationShareDirection = "from_friend"
	ShareMutual     LocationShareDirection = "mutual"
)

// User is a user account with authentication and profile data.
type User struct {
	ID           int64    `json:"id"`
	Email        string   `json:"email"`
	EmailNorm    string   `json:"-"` // normalized email used for lookups
	PasswordHash string   `json:"-"`
	FullName     string   `json:"full_name"`
	Role         UserRole `json:"role"`

	// Account security
	IsActive            bool       `json:"is_active"`
	IsLocked            bool       `json:"-"`
	LockedAt            *time.Time `json:"-"`
	FailedLoginAttempts int        `json:"-"`
	MustChangePassword  bool       `json:"must_change_password"`

	CreatedAt   time.Time  `json:"created_at"`
	UpdatedAt   *time.Time `json:"-"`
	LastLoginAt *time.Time `json:"last_login_at"`
}

// Session is a server-side session tracking active logins.
type Session struct {
	ID             int64      `json:"id"`
	SessionToken   string     `json:"-"`
	CSRFToken      string     `json:"-"` // double-submit CSRF token for this session
	UserID         int64      `json:"user_id"`
	IPAddress      string     `json:"-"`
	UserAgent      string     `json:"-"`
	CreatedAt      time.Time  `json:"-"`
	ExpiresAt      time.Time  `json:"-"`
	LastActivityAt time.Time  `json:"-"`
	RevokedAt      *time.Time `json:"-"`
}

// IsValid reports whether the session can still authenticate its owner.
func (s *Session) IsValid(u *User, now time.Time) bool {
	return s.RevokedAt == nil &&
		s.ExpiresAt.After(now) &&
		u != nil && u.IsActive && !u.IsLocked
}

// Friendship links two users with a status.
type Friendship struct {
	ID          int64            `json:"id"`
	RequesterID int64            `json:"requester_id"`
	ReceiverID  int64            `json:"receiver_id"`
	Status      FriendshipStatus `json:"status"`

	CreatedAt   time.Time  `json:"created_at"`
	UpdatedAt   *time.Time `json:"-"`
	RespondedAt *time.Time `json:"responded_at"`
}

// LocationShare is a directional location-sharing permission tied to a
// friendship and owned by one of the two users.
type LocationShare struct {
	ID           int64                  `json:"id"`
	FriendshipID int64                  `json:"friendship_id"`
	OwnerID      int64                  `json:"owner_id"`
	Direction    LocationShareDirection `json:"direction"`

	IsActive           bool       `json:"is_active"`
	PrecisionLevel     string     `json:"precision_level"` // exact|approximate|city_only
	CreatedAt          time.Time  `json:"created_at"`
	UpdatedAt          *time.Time `json:"-"`
	LastLocationUpdate *time.Time `json:"last_location_update_at"`

	// Latest known coordinates (privacy-sensitive; never serialized directly).
	HasLocation bool
	Latitude    float64
	Longitude   float64
	Accuracy    *float64
}

// LoginAttempt records an authentication attempt for auditing.
type LoginAttempt struct {
	ID          int64     `json:"id"`
	Email       string    `json:"email"`
	IPAddress   string    `json:"ip_address"`
	Success     bool      `json:"success"`
	AttemptedAt time.Time `json:"attempted_at"`
}
