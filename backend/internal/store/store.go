// Package store provides the persistence layer for the Always Together API.
// It is a thread-safe in-memory implementation of all entity tables
// (users, sessions, friendships, location shares, login attempts).
//
// The public API is intentionally repository-style so it can be swapped for a
// PostgreSQL-backed implementation without touching the HTTP handlers.
package store

import (
	"errors"
	"strings"
	"sync"
	"time"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

// ErrNotFound is returned when an entity does not exist.
var ErrNotFound = errors.New("store: not found")

// Store holds all application entities in memory.
type Store struct {
	mu sync.RWMutex

	users        map[int64]*models.User
	usersByEmail map[string]int64 // normalized email -> user id
	sessions     map[string]*models.Session
	friendships  map[int64]*models.Friendship
	shares       map[int64]*models.LocationShare
	attempts     []models.LoginAttempt

	nextUserID       int64
	nextSessionID    int64
	nextFriendshipID int64
	nextShareID      int64
}

// New creates an empty store.
func New() *Store {
	return &Store{
		users:        make(map[int64]*models.User),
		usersByEmail: make(map[string]int64),
		sessions:     make(map[string]*models.Session),
		friendships:  make(map[int64]*models.Friendship),
		shares:       make(map[int64]*models.LocationShare),
	}
}

// nowUTC returns the current UTC time truncated to microsecond precision.
// PostgreSQL TIMESTAMPTZ stores microseconds, so truncating here keeps
// round-tripped timestamps Equal to freshly created ones on both drivers.
func nowUTC() time.Time { return time.Now().UTC().Truncate(time.Microsecond) }

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

// CreateUser inserts a new user, enforcing unique (normalized) email.
func (s *Store) CreateUser(u *models.User) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	norm := strings.ToLower(strings.TrimSpace(u.Email))
	if _, exists := s.usersByEmail[norm]; exists {
		return errors.New("store: email already registered")
	}

	s.nextUserID++
	u.ID = s.nextUserID
	u.EmailNorm = norm
	if u.Role == "" {
		u.Role = models.RoleResident
	}
	u.IsActive = true
	u.CreatedAt = nowUTC()

	cp := *u
	s.users[u.ID] = &cp
	s.usersByEmail[norm] = u.ID
	return nil
}

// GetUserByID returns the user with the given id.
func (s *Store) GetUserByID(id int64) (*models.User, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	u, ok := s.users[id]
	if !ok {
		return nil, ErrNotFound
	}
	cp := *u
	return &cp, nil
}

// GetUserByEmail looks a user up by normalized email.
func (s *Store) GetUserByEmail(normEmail string) (*models.User, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	id, ok := s.usersByEmail[strings.ToLower(strings.TrimSpace(normEmail))]
	if !ok {
		return nil, ErrNotFound
	}
	u := s.users[id]
	cp := *u
	return &cp, nil
}

// ListUsers returns all users.
func (s *Store) ListUsers() []models.User {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]models.User, 0, len(s.users))
	for _, u := range s.users {
		out = append(out, *u)
	}
	return out
}

// UpdateUser applies changes to a stored user.
func (s *Store) UpdateUser(u *models.User) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	existing, ok := s.users[u.ID]
	if !ok {
		return ErrNotFound
	}
	oldEmailNorm := existing.EmailNorm
	t := nowUTC()
	cp := *u
	cp.CreatedAt = existing.CreatedAt
	cp.LastLoginAt = existing.LastLoginAt
	cp.UpdatedAt = &t
	cp.EmailNorm = strings.ToLower(strings.TrimSpace(cp.Email))
	*existing = cp
	if oldEmailNorm != cp.EmailNorm {
		delete(s.usersByEmail, oldEmailNorm)
		s.usersByEmail[cp.EmailNorm] = cp.ID
	}
	return nil
}

// SetUserActive enables or disables an account (admin action). Deactivating a
// user also revokes all of their active sessions.
func (s *Store) SetUserActive(id int64, active bool) error {
	s.mu.Lock()
	user, ok := s.users[id]
	if !ok {
		s.mu.Unlock()
		return ErrNotFound
	}
	user.IsActive = active
	token := user.EmailNorm
	var revoke []string
	if !active {
		for tok, sess := range s.sessions {
			if sess.UserID == id && sess.RevokedAt == nil {
				revoke = append(revoke, tok)
			}
		}
		for _, tok := range revoke {
			rev := *s.sessions[tok]
			now := nowUTC()
			rev.RevokedAt = &now
			s.sessions[tok] = &rev
		}
	}
	s.mu.Unlock()
	_ = token
	return nil
}

// UnlockUser clears the failed-login counter and lock timestamp so the next
// login attempt is evaluated normally (the in-memory lockout window is cleared
// separately by the handler).
func (s *Store) UnlockUser(id int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	user, ok := s.users[id]
	if !ok {
		return ErrNotFound
	}
	user.IsLocked = false
	user.FailedLoginAttempts = 0
	user.LockedAt = nil
	return nil
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

// CreateSession persists a new server-side session.
func (s *Store) CreateSession(sess *models.Session) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.users[sess.UserID]; !ok {
		return errors.New("store: unknown user for session")
	}
	s.nextSessionID++
	sess.ID = s.nextSessionID
	if sess.CSRFToken == "" {
		sess.CSRFToken = security.GenerateCSRFToken()
	}
	sess.CreatedAt = nowUTC()
	sess.LastActivityAt = sess.CreatedAt

	cp := *sess
	s.sessions[sess.SessionToken] = &cp
	return nil
}

// GetSession retrieves a session by token.
func (s *Store) GetSession(token string) (*models.Session, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	sess, ok := s.sessions[token]
	if !ok {
		return nil, ErrNotFound
	}
	cp := *sess
	return &cp, nil
}

// RevokeSession marks a single session as revoked.
func (s *Store) RevokeSession(token string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if sess, ok := s.sessions[token]; ok {
		t := nowUTC()
		sess.RevokedAt = &t
	}
}

// RevokeAllSessionsForUser revokes every active session of a user (used on
// password change).
func (s *Store) RevokeAllSessionsForUser(userID int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	t := nowUTC()
	for _, sess := range s.sessions {
		if sess.UserID == userID && sess.RevokedAt == nil {
			sess.RevokedAt = &t
		}
	}
}

// TouchSession updates last activity (idle-timeout support).
func (s *Store) TouchSession(token string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if sess, ok := s.sessions[token]; ok {
		sess.LastActivityAt = nowUTC()
	}
}

// ---------------------------------------------------------------------------
// Login attempts
// ---------------------------------------------------------------------------

// RecordLoginAttempt stores an authentication attempt for auditing.
func (s *Store) RecordLoginAttempt(email, ip string, success bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.attempts = append(s.attempts, models.LoginAttempt{
		ID:          int64(len(s.attempts) + 1),
		Email:       email,
		IPAddress:   ip,
		Success:     success,
		AttemptedAt: nowUTC(),
	})
}

// RecentFailedAttempts counts failed login attempts for an email within the
// given window.
func (s *Store) RecentFailedAttempts(email string, window time.Duration) int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	cutoff := nowUTC().Add(-window)
	n := 0
	for _, a := range s.attempts {
		if !a.Success && strings.EqualFold(a.Email, email) && a.AttemptedAt.After(cutoff) {
			n++
		}
	}
	return n
}

// ---------------------------------------------------------------------------
// Friendships
// ---------------------------------------------------------------------------

// CreateFriendship inserts a pending friendship request.
func (s *Store) CreateFriendship(f *models.Friendship) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Prevent duplicate pending friendships between the same pair.
	for _, existing := range s.friendships {
		samePair := (existing.RequesterID == f.RequesterID && existing.ReceiverID == f.ReceiverID) ||
			(existing.RequesterID == f.ReceiverID && existing.ReceiverID == f.RequesterID)
		if samePair && existing.Status == models.FriendshipPending {
			return errors.New("store: friendship request already exists")
		}
	}

	s.nextFriendshipID++
	f.ID = s.nextFriendshipID
	if f.Status == "" {
		f.Status = models.FriendshipPending
	}
	f.CreatedAt = nowUTC()

	cp := *f
	s.friendships[f.ID] = &cp
	return nil
}

// GetFriendship returns a friendship by id.
func (s *Store) GetFriendship(id int64) (*models.Friendship, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	f, ok := s.friendships[id]
	if !ok {
		return nil, ErrNotFound
	}
	cp := *f
	return &cp, nil
}

// UpdateFriendship persists changes to a friendship.
func (s *Store) UpdateFriendship(f *models.Friendship) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	existing, ok := s.friendships[f.ID]
	if !ok {
		return ErrNotFound
	}
	t := nowUTC()
	f.UpdatedAt = &t
	cp := *f
	*existing = cp
	return nil
}

// ListFriendshipsForUser returns every friendship involving the user.
func (s *Store) ListFriendshipsForUser(userID int64) []models.Friendship {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := []models.Friendship{}
	for _, f := range s.friendships {
		if f.RequesterID == userID || f.ReceiverID == userID {
			out = append(out, *f)
		}
	}
	return out
}

// ---------------------------------------------------------------------------
// Location shares
// ---------------------------------------------------------------------------

// CreateLocationShare inserts a directional sharing permission.
func (s *Store) CreateLocationShare(ls *models.LocationShare) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.friendships[ls.FriendshipID]; !ok {
		return errors.New("store: unknown friendship for location share")
	}
	s.nextShareID++
	ls.ID = s.nextShareID
	ls.CreatedAt = nowUTC()

	cp := *ls
	s.shares[ls.ID] = &cp
	return nil
}

// GetLocationShare returns a share by id.
func (s *Store) GetLocationShare(id int64) (*models.LocationShare, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	ls, ok := s.shares[id]
	if !ok {
		return nil, ErrNotFound
	}
	cp := *ls
	return &cp, nil
}

// UpdateLocationShare persists changes to a share.
func (s *Store) UpdateLocationShare(ls *models.LocationShare) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	existing, ok := s.shares[ls.ID]
	if !ok {
		return ErrNotFound
	}
	t := nowUTC()
	ls.UpdatedAt = &t
	cp := *ls
	*existing = cp
	return nil
}

// ListLocationSharesByOwner returns all shares owned by a user.
func (s *Store) ListLocationSharesByOwner(ownerID int64) []models.LocationShare {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := []models.LocationShare{}
	for _, ls := range s.shares {
		if ls.OwnerID == ownerID {
			out = append(out, *ls)
		}
	}
	return out
}

// ListActiveSharesVisibleTo returns the owner's active shares that grant the
// viewer permission to see the owner's location (to_friend or mutual).
func (s *Store) ListActiveSharesVisibleTo(ownerID, viewerID int64) []models.LocationShare {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := []models.LocationShare{}
	for _, ls := range s.shares {
		if ls.OwnerID != ownerID || !ls.IsActive {
			continue
		}
		f, ok := s.friendships[ls.FriendshipID]
		if !ok || f.Status != models.FriendshipAccepted {
			continue
		}
		other := f.RequesterID
		if other == ownerID {
			other = f.ReceiverID
		}
		if other != viewerID {
			continue
		}
		if ls.Direction == models.ShareToFriend || ls.Direction == models.ShareMutual {
			out = append(out, *ls)
		}
	}
	return out
}
