// PostgreSQL-backed Repository (T01).
//
// Implements the full store.Repository surface over database/sql with the
// pgx stdlib driver. Semantics mirror the in-memory Store exactly (error
// strings included) so the conformance suite passes against both drivers.
//
// Schema bootstrap uses CREATE TABLE IF NOT EXISTS for a zero-dependency
// first boot; T02 replaces this with goose-managed migrations.
package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"

	_ "github.com/jackc/pgx/v5/stdlib"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

// Postgres is a Repository backed by PostgreSQL.
type Postgres struct {
	db *sql.DB
}

// OpenPostgres connects to PostgreSQL, configures the pool, and ensures the
// schema exists. The returned store is ready for use.
func OpenPostgres(databaseURL string) (*Postgres, error) {
	db, err := sql.Open("pgx", databaseURL)
	if err != nil {
		return nil, fmt.Errorf("store: open postgres: %w", err)
	}

	// Conservative pool defaults; deployments tune via DATABASE_URL params.
	db.SetMaxOpenConns(20)
	db.SetMaxIdleConns(10)
	db.SetConnMaxLifetime(30 * time.Minute)
	db.SetConnMaxIdleTime(5 * time.Minute)

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := db.PingContext(ctx); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("store: ping postgres: %w", err)
	}

	p := &Postgres{db: db}
	if err := p.ensureSchema(ctx); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("store: ensure schema: %w", err)
	}
	return p, nil
}

// Close releases the connection pool.
func (p *Postgres) Close() error { return p.db.Close() }

// DB exposes the underlying pool (health checks, future migrations).
func (p *Postgres) DB() *sql.DB { return p.db }

var schemaStatements = []string{
	`CREATE TABLE IF NOT EXISTS users (
		id BIGSERIAL PRIMARY KEY,
		email TEXT NOT NULL UNIQUE,
		email_norm TEXT NOT NULL UNIQUE,
		password_hash TEXT NOT NULL,
		full_name TEXT NOT NULL DEFAULT '',
		role TEXT NOT NULL DEFAULT 'resident',
		is_active BOOLEAN NOT NULL DEFAULT TRUE,
		is_locked BOOLEAN NOT NULL DEFAULT FALSE,
		locked_at TIMESTAMPTZ,
		failed_login_attempts INT NOT NULL DEFAULT 0,
		must_change_password BOOLEAN NOT NULL DEFAULT FALSE,
		created_at TIMESTAMPTZ NOT NULL,
		updated_at TIMESTAMPTZ,
		last_login_at TIMESTAMPTZ
	)`,
	`CREATE TABLE IF NOT EXISTS sessions (
		id BIGSERIAL PRIMARY KEY,
		session_token TEXT NOT NULL UNIQUE,
		csrf_token TEXT NOT NULL DEFAULT '',
		user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
		ip_address TEXT NOT NULL DEFAULT '',
		user_agent TEXT NOT NULL DEFAULT '',
		created_at TIMESTAMPTZ NOT NULL,
		expires_at TIMESTAMPTZ NOT NULL,
		last_activity_at TIMESTAMPTZ NOT NULL,
		revoked_at TIMESTAMPTZ
	)`,
	`CREATE TABLE IF NOT EXISTS friendships (
		id BIGSERIAL PRIMARY KEY,
		requester_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
		receiver_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
		status TEXT NOT NULL DEFAULT 'pending',
		created_at TIMESTAMPTZ NOT NULL,
		updated_at TIMESTAMPTZ,
		responded_at TIMESTAMPTZ
	)`,
	`CREATE TABLE IF NOT EXISTS location_shares (
		id BIGSERIAL PRIMARY KEY,
		friendship_id BIGINT NOT NULL REFERENCES friendships(id) ON DELETE CASCADE,
		owner_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
		direction TEXT NOT NULL DEFAULT 'to_friend',
		is_active BOOLEAN NOT NULL DEFAULT TRUE,
		precision_level TEXT NOT NULL DEFAULT 'exact',
		created_at TIMESTAMPTZ NOT NULL,
		updated_at TIMESTAMPTZ,
		last_location_update TIMESTAMPTZ,
		has_location BOOLEAN NOT NULL DEFAULT FALSE,
		latitude DOUBLE PRECISION,
		longitude DOUBLE PRECISION,
		accuracy DOUBLE PRECISION
	)`,
	`CREATE TABLE IF NOT EXISTS login_attempts (
		id BIGSERIAL PRIMARY KEY,
		email TEXT NOT NULL,
		ip_address TEXT NOT NULL DEFAULT '',
		success BOOLEAN NOT NULL,
		attempted_at TIMESTAMPTZ NOT NULL
	)`,
	`CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id)`,
	`CREATE INDEX IF NOT EXISTS idx_friendships_requester ON friendships(requester_id)`,
	`CREATE INDEX IF NOT EXISTS idx_friendships_receiver ON friendships(receiver_id)`,
	`CREATE INDEX IF NOT EXISTS idx_shares_owner ON location_shares(owner_id)`,
	`CREATE INDEX IF NOT EXISTS idx_shares_friendship ON location_shares(friendship_id)`,
	`CREATE INDEX IF NOT EXISTS idx_login_attempts_email_time ON login_attempts(email, attempted_at)`,
}

func (p *Postgres) ensureSchema(ctx context.Context) error {
	for _, stmt := range schemaStatements {
		if _, err := p.db.ExecContext(ctx, stmt); err != nil {
			return fmt.Errorf("%w: %s", err, truncate(stmt, 60))
		}
	}
	return nil
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}

// errEmailTaken matches the in-memory store's message so API responses are
// driver-independent.
var errEmailTaken = errors.New("store: email already registered")

// Compile-time check: the Postgres driver satisfies the full repository.
var _ Repository = (*Postgres)(nil)

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

// CreateUser inserts a new user, enforcing unique (normalized) email.
func (p *Postgres) CreateUser(u *models.User) error {
	norm := strings.ToLower(strings.TrimSpace(u.Email))
	if u.Role == "" {
		u.Role = models.RoleResident
	}
	u.EmailNorm = norm
	u.IsActive = true
	u.CreatedAt = nowUTC()

	row := p.db.QueryRow(`
		INSERT INTO users (email, email_norm, password_hash, full_name, role,
			is_active, is_locked, locked_at, failed_login_attempts,
			must_change_password, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
		RETURNING id`,
		u.Email, u.EmailNorm, u.PasswordHash, u.FullName, u.Role,
		u.IsActive, u.IsLocked, u.LockedAt, u.FailedLoginAttempts,
		u.MustChangePassword, u.CreatedAt,
	)
	if err := row.Scan(&u.ID); err != nil {
		if isUniqueViolation(err) {
			return errEmailTaken
		}
		return fmt.Errorf("store: create user: %w", err)
	}
	return nil
}

// GetUserByID returns the user with the given id.
func (p *Postgres) GetUserByID(id int64) (*models.User, error) {
	return p.scanUser(p.db.QueryRow(`
		SELECT id, email, email_norm, password_hash, full_name, role,
			is_active, is_locked, locked_at, failed_login_attempts,
			must_change_password, created_at, updated_at, last_login_at
		FROM users WHERE id = $1`, id))
}

// GetUserByEmail looks a user up by normalized email.
func (p *Postgres) GetUserByEmail(normEmail string) (*models.User, error) {
	return p.scanUser(p.db.QueryRow(`
		SELECT id, email, email_norm, password_hash, full_name, role,
			is_active, is_locked, locked_at, failed_login_attempts,
			must_change_password, created_at, updated_at, last_login_at
		FROM users WHERE email_norm = $1`,
		strings.ToLower(strings.TrimSpace(normEmail))))
}

func (p *Postgres) scanUser(row *sql.Row) (*models.User, error) {
	u := &models.User{}
	err := row.Scan(&u.ID, &u.Email, &u.EmailNorm, &u.PasswordHash, &u.FullName,
		&u.Role, &u.IsActive, &u.IsLocked, &u.LockedAt, &u.FailedLoginAttempts,
		&u.MustChangePassword, &u.CreatedAt, &u.UpdatedAt, &u.LastLoginAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, fmt.Errorf("store: get user: %w", err)
	}
	return u, nil
}

// ListUsers returns all users.
func (p *Postgres) ListUsers() []models.User {
	rows, err := p.db.Query(`
		SELECT id, email, email_norm, password_hash, full_name, role,
			is_active, is_locked, locked_at, failed_login_attempts,
			must_change_password, created_at, updated_at, last_login_at
		FROM users ORDER BY id`)
	if err != nil {
		return []models.User{}
	}
	defer rows.Close()
	return scanUsers(rows)
}

func scanUsers(rows *sql.Rows) []models.User {
	out := []models.User{}
	for rows.Next() {
		var u models.User
		if err := rows.Scan(&u.ID, &u.Email, &u.EmailNorm, &u.PasswordHash,
			&u.FullName, &u.Role, &u.IsActive, &u.IsLocked, &u.LockedAt,
			&u.FailedLoginAttempts, &u.MustChangePassword, &u.CreatedAt,
			&u.UpdatedAt, &u.LastLoginAt); err != nil {
			return out
		}
		out = append(out, u)
	}
	return out
}

// UpdateUser applies changes to a stored user, preserving creation time and
// last login, and refreshing the normalized email index.
func (p *Postgres) UpdateUser(u *models.User) error {
	u.EmailNorm = strings.ToLower(strings.TrimSpace(u.Email))
	t := nowUTC()
	u.UpdatedAt = &t
	res, err := p.db.Exec(`
		UPDATE users SET email=$1, email_norm=$2, password_hash=$3, full_name=$4,
			role=$5, is_active=$6, is_locked=$7, locked_at=$8,
			failed_login_attempts=$9, must_change_password=$10, updated_at=$11
		WHERE id=$12`,
		u.Email, u.EmailNorm, u.PasswordHash, u.FullName, u.Role, u.IsActive,
		u.IsLocked, u.LockedAt, u.FailedLoginAttempts, u.MustChangePassword,
		u.UpdatedAt, u.ID)
	if err != nil {
		if isUniqueViolation(err) {
			return errEmailTaken
		}
		return fmt.Errorf("store: update user: %w", err)
	}
	return requireAffected(res)
}

// SetUserActive enables or disables an account (admin action). Deactivating a
// user also revokes all of their active sessions.
func (p *Postgres) SetUserActive(id int64, active bool) error {
	res, err := p.db.Exec(`UPDATE users SET is_active=$1 WHERE id=$2`, active, id)
	if err != nil {
		return fmt.Errorf("store: set active: %w", err)
	}
	if err := requireAffected(res); err != nil {
		return err
	}
	if !active {
		if _, err := p.db.Exec(
			`UPDATE sessions SET revoked_at=$1 WHERE user_id=$2 AND revoked_at IS NULL`,
			nowUTC(), id); err != nil {
			return fmt.Errorf("store: revoke sessions: %w", err)
		}
	}
	return nil
}

// UnlockUser clears the failed-login counter and lock timestamp.
func (p *Postgres) UnlockUser(id int64) error {
	res, err := p.db.Exec(`
		UPDATE users SET is_locked=FALSE, failed_login_attempts=0, locked_at=NULL
		WHERE id=$1`, id)
	if err != nil {
		return fmt.Errorf("store: unlock user: %w", err)
	}
	return requireAffected(res)
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

// CreateSession persists a new server-side session.
func (p *Postgres) CreateSession(sess *models.Session) error {
	var userExists bool
	if err := p.db.QueryRow(
		`SELECT EXISTS(SELECT 1 FROM users WHERE id=$1)`, sess.UserID,
	).Scan(&userExists); err != nil {
		return fmt.Errorf("store: session user check: %w", err)
	}
	if !userExists {
		return errors.New("store: unknown user for session")
	}
	if sess.CSRFToken == "" {
		sess.CSRFToken = security.GenerateCSRFToken()
	}
	sess.CreatedAt = nowUTC()
	sess.LastActivityAt = sess.CreatedAt

	row := p.db.QueryRow(`
		INSERT INTO sessions (session_token, csrf_token, user_id, ip_address,
			user_agent, created_at, expires_at, last_activity_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
		RETURNING id`,
		sess.SessionToken, sess.CSRFToken, sess.UserID, sess.IPAddress,
		sess.UserAgent, sess.CreatedAt, sess.ExpiresAt, sess.LastActivityAt)
	if err := row.Scan(&sess.ID); err != nil {
		return fmt.Errorf("store: create session: %w", err)
	}
	return nil
}

// GetSession retrieves a session by token.
func (p *Postgres) GetSession(token string) (*models.Session, error) {
	sess := &models.Session{}
	err := p.db.QueryRow(`
		SELECT id, session_token, csrf_token, user_id, ip_address, user_agent,
			created_at, expires_at, last_activity_at, revoked_at
		FROM sessions WHERE session_token=$1`, token,
	).Scan(&sess.ID, &sess.SessionToken, &sess.CSRFToken, &sess.UserID,
		&sess.IPAddress, &sess.UserAgent, &sess.CreatedAt, &sess.ExpiresAt,
		&sess.LastActivityAt, &sess.RevokedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, fmt.Errorf("store: get session: %w", err)
	}
	return sess, nil
}

// RevokeSession marks a single session as revoked.
func (p *Postgres) RevokeSession(token string) {
	_, _ = p.db.Exec(`UPDATE sessions SET revoked_at=$1 WHERE session_token=$2`,
		nowUTC(), token)
}

// RevokeAllSessionsForUser revokes every active session of a user (used on
// password change).
func (p *Postgres) RevokeAllSessionsForUser(userID int64) {
	_, _ = p.db.Exec(`
		UPDATE sessions SET revoked_at=$1 WHERE user_id=$2 AND revoked_at IS NULL`,
		nowUTC(), userID)
}

// TouchSession updates last activity (idle-timeout support).
func (p *Postgres) TouchSession(token string) {
	_, _ = p.db.Exec(`UPDATE sessions SET last_activity_at=$1 WHERE session_token=$2`,
		nowUTC(), token)
}

// ---------------------------------------------------------------------------
// Login attempts
// ---------------------------------------------------------------------------

// RecordLoginAttempt stores an authentication attempt for auditing. The email
// is normalized (lowercased) so case-insensitive lookups work, mirroring the
// in-memory driver.
func (p *Postgres) RecordLoginAttempt(email, ip string, success bool) {
	_, _ = p.db.Exec(`
		INSERT INTO login_attempts (email, ip_address, success, attempted_at)
		VALUES ($1, $2, $3, $4)`,
		strings.ToLower(strings.TrimSpace(email)), ip, success, nowUTC())
}

// RecentFailedAttempts counts failed login attempts for an email within the
// given window. Matching is case-insensitive.
func (p *Postgres) RecentFailedAttempts(email string, window time.Duration) int {
	var n int
	err := p.db.QueryRow(`
		SELECT COUNT(*) FROM login_attempts
		WHERE success=FALSE AND email=$1 AND attempted_at > $2`,
		strings.ToLower(strings.TrimSpace(email)), nowUTC().Add(-window)).Scan(&n)
	if err != nil {
		return 0
	}
	return n
}

// ---------------------------------------------------------------------------
// Friendships
// ---------------------------------------------------------------------------

// CreateFriendship inserts a pending friendship request, preventing duplicate
// pending requests between the same pair (in either direction).
func (p *Postgres) CreateFriendship(f *models.Friendship) error {
	var duplicate bool
	if err := p.db.QueryRow(`
		SELECT EXISTS(
			SELECT 1 FROM friendships
			WHERE status='pending' AND
				((requester_id=$1 AND receiver_id=$2) OR
				 (requester_id=$2 AND receiver_id=$1)))`,
		f.RequesterID, f.ReceiverID).Scan(&duplicate); err != nil {
		return fmt.Errorf("store: friendship duplicate check: %w", err)
	}
	if duplicate {
		return errors.New("store: friendship request already exists")
	}
	if f.Status == "" {
		f.Status = models.FriendshipPending
	}
	f.CreatedAt = nowUTC()

	row := p.db.QueryRow(`
		INSERT INTO friendships (requester_id, receiver_id, status, created_at,
			responded_at)
		VALUES ($1, $2, $3, $4, $5) RETURNING id`,
		f.RequesterID, f.ReceiverID, f.Status, f.CreatedAt, f.RespondedAt)
	if err := row.Scan(&f.ID); err != nil {
		return fmt.Errorf("store: create friendship: %w", err)
	}
	return nil
}

// GetFriendship returns a friendship by id.
func (p *Postgres) GetFriendship(id int64) (*models.Friendship, error) {
	f := &models.Friendship{}
	err := p.db.QueryRow(`
		SELECT id, requester_id, receiver_id, status, created_at, updated_at,
			responded_at
		FROM friendships WHERE id=$1`, id,
	).Scan(&f.ID, &f.RequesterID, &f.ReceiverID, &f.Status, &f.CreatedAt,
		&f.UpdatedAt, &f.RespondedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, fmt.Errorf("store: get friendship: %w", err)
	}
	return f, nil
}

// UpdateFriendship persists changes to a friendship.
func (p *Postgres) UpdateFriendship(f *models.Friendship) error {
	t := nowUTC()
	f.UpdatedAt = &t
	res, err := p.db.Exec(`
		UPDATE friendships SET status=$1, updated_at=$2, responded_at=$3
		WHERE id=$4`, f.Status, f.UpdatedAt, f.RespondedAt, f.ID)
	if err != nil {
		return fmt.Errorf("store: update friendship: %w", err)
	}
	return requireAffected(res)
}

// ListFriendshipsForUser returns every friendship involving the user.
func (p *Postgres) ListFriendshipsForUser(userID int64) []models.Friendship {
	rows, err := p.db.Query(`
		SELECT id, requester_id, receiver_id, status, created_at, updated_at,
			responded_at
		FROM friendships
		WHERE requester_id=$1 OR receiver_id=$1
		ORDER BY id`, userID)
	if err != nil {
		return []models.Friendship{}
	}
	defer rows.Close()
	out := []models.Friendship{}
	for rows.Next() {
		var f models.Friendship
		if err := rows.Scan(&f.ID, &f.RequesterID, &f.ReceiverID, &f.Status,
			&f.CreatedAt, &f.UpdatedAt, &f.RespondedAt); err != nil {
			return out
		}
		out = append(out, f)
	}
	return out
}

// ---------------------------------------------------------------------------
// Location shares
// ---------------------------------------------------------------------------

// CreateLocationShare inserts a directional sharing permission.
func (p *Postgres) CreateLocationShare(ls *models.LocationShare) error {
	var friendshipExists bool
	if err := p.db.QueryRow(
		`SELECT EXISTS(SELECT 1 FROM friendships WHERE id=$1)`, ls.FriendshipID,
	).Scan(&friendshipExists); err != nil {
		return fmt.Errorf("store: share friendship check: %w", err)
	}
	if !friendshipExists {
		return errors.New("store: unknown friendship for location share")
	}
	ls.CreatedAt = nowUTC()

	row := p.db.QueryRow(`
		INSERT INTO location_shares (friendship_id, owner_id, direction,
			is_active, precision_level, created_at, last_location_update,
			has_location, latitude, longitude, accuracy)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
		RETURNING id`,
		ls.FriendshipID, ls.OwnerID, ls.Direction, ls.IsActive,
		ls.PrecisionLevel, ls.CreatedAt, ls.LastLocationUpdate, ls.HasLocation,
		nullableFloat(ls.Latitude, ls.HasLocation),
		nullableFloat(ls.Longitude, ls.HasLocation),
		ls.Accuracy)
	if err := row.Scan(&ls.ID); err != nil {
		return fmt.Errorf("store: create share: %w", err)
	}
	return nil
}

// GetLocationShare returns a share by id.
func (p *Postgres) GetLocationShare(id int64) (*models.LocationShare, error) {
	ls, err := scanShareRow(p.db.QueryRow(`
		SELECT id, friendship_id, owner_id, direction, is_active,
			precision_level, created_at, updated_at, last_location_update,
			has_location, latitude, longitude, accuracy
		FROM location_shares WHERE id=$1`, id))
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, fmt.Errorf("store: get share: %w", err)
	}
	return ls, nil
}

// UpdateLocationShare persists changes to a share.
func (p *Postgres) UpdateLocationShare(ls *models.LocationShare) error {
	t := nowUTC()
	ls.UpdatedAt = &t
	res, err := p.db.Exec(`
		UPDATE location_shares SET direction=$1, is_active=$2,
			precision_level=$3, updated_at=$4, last_location_update=$5,
			has_location=$6, latitude=$7, longitude=$8, accuracy=$9
		WHERE id=$10`,
		ls.Direction, ls.IsActive, ls.PrecisionLevel, ls.UpdatedAt,
		ls.LastLocationUpdate, ls.HasLocation,
		nullableFloat(ls.Latitude, ls.HasLocation),
		nullableFloat(ls.Longitude, ls.HasLocation),
		ls.Accuracy, ls.ID)
	if err != nil {
		return fmt.Errorf("store: update share: %w", err)
	}
	return requireAffected(res)
}

// ListLocationSharesByOwner returns all shares owned by a user.
func (p *Postgres) ListLocationSharesByOwner(ownerID int64) []models.LocationShare {
	rows, err := p.db.Query(`
		SELECT id, friendship_id, owner_id, direction, is_active,
			precision_level, created_at, updated_at, last_location_update,
			has_location, latitude, longitude, accuracy
		FROM location_shares WHERE owner_id=$1 ORDER BY id`, ownerID)
	if err != nil {
		return []models.LocationShare{}
	}
	defer rows.Close()
	return scanShares(rows)
}

// ListActiveSharesVisibleTo returns the owner's active shares that grant the
// viewer permission to see the owner's location (to_friend or mutual) through
// an accepted friendship.
func (p *Postgres) ListActiveSharesVisibleTo(ownerID, viewerID int64) []models.LocationShare {
	rows, err := p.db.Query(`
		SELECT s.id, s.friendship_id, s.owner_id, s.direction, s.is_active,
			s.precision_level, s.created_at, s.updated_at,
			s.last_location_update, s.has_location, s.latitude, s.longitude,
			s.accuracy
		FROM location_shares s
		JOIN friendships f ON f.id = s.friendship_id
		WHERE s.owner_id=$1 AND s.is_active=TRUE AND f.status='accepted'
			AND ((f.requester_id=$2 AND f.receiver_id=$1)
				OR (f.requester_id=$1 AND f.receiver_id=$2))
			AND s.direction IN ('to_friend', 'mutual')
		ORDER BY s.id`, ownerID, viewerID)
	if err != nil {
		return []models.LocationShare{}
	}
	defer rows.Close()
	return scanShares(rows)
}

func scanShares(rows *sql.Rows) []models.LocationShare {
	out := []models.LocationShare{}
	for rows.Next() {
		ls, err := scanShareRow(rows)
		if err != nil {
			return out
		}
		out = append(out, *ls)
	}
	return out
}

// scanShareRow reads one location_share row. latitude/longitude are nullable
// in the schema (a share can exist before any location arrives) but the model
// uses plain float64, so NULL maps to the zero value, matching the in-memory
// driver.
func scanShareRow(sc rowScanner) (*models.LocationShare, error) {
	ls := &models.LocationShare{}
	var lat, lon sql.NullFloat64
	if err := sc.Scan(&ls.ID, &ls.FriendshipID, &ls.OwnerID, &ls.Direction,
		&ls.IsActive, &ls.PrecisionLevel, &ls.CreatedAt, &ls.UpdatedAt,
		&ls.LastLocationUpdate, &ls.HasLocation, &lat, &lon, &ls.Accuracy,
	); err != nil {
		return nil, err
	}
	if lat.Valid {
		ls.Latitude = lat.Float64
	}
	if lon.Valid {
		ls.Longitude = lon.Float64
	}
	return ls, nil
}

// rowScanner abstracts *sql.Row and *sql.Rows for the share scanner.
type rowScanner interface {
	Scan(dest ...any) error
}

// ---------------------------------------------------------------------------
// Seed
// ---------------------------------------------------------------------------

// SeedDemoData populates the store with an administrator and two accepted
// friends with mutual location sharing enabled. Idempotent: no-op when users
// already exist.
func (p *Postgres) SeedDemoData() bool {
	var count int
	if err := p.db.QueryRow(`SELECT COUNT(*) FROM users`).Scan(&count); err != nil {
		return false
	}
	if count > 0 {
		return false
	}

	hashAdmin, err := security.GetPasswordHash("ChangeMe123!")
	if err != nil {
		return false
	}
	hashAlice, err := security.GetPasswordHash("DemoPass123!")
	if err != nil {
		return false
	}
	hashBob, err := security.GetPasswordHash("DemoPass456!")
	if err != nil {
		return false
	}

	admin := &models.User{
		Email: "admin@example.com", PasswordHash: hashAdmin,
		FullName: "Administrator", Role: models.RoleAdministrator,
		IsActive: true, MustChangePassword: true,
	}
	alice := &models.User{
		Email: "alice@example.com", PasswordHash: hashAlice,
		FullName: "Alice Example", Role: models.RoleResident, IsActive: true,
	}
	bob := &models.User{
		Email: "bob@example.com", PasswordHash: hashBob,
		FullName: "Bob Example", Role: models.RoleResident, IsActive: true,
	}
	for _, u := range []*models.User{admin, alice, bob} {
		if err := p.CreateUser(u); err != nil {
			return false
		}
	}

	now := nowUTC()
	f := &models.Friendship{
		RequesterID: alice.ID, ReceiverID: bob.ID,
		Status: models.FriendshipAccepted, CreatedAt: now, RespondedAt: &now,
	}
	if err := p.CreateFriendship(f); err != nil {
		return false
	}

	shares := []*models.LocationShare{
		{FriendshipID: f.ID, OwnerID: alice.ID, Direction: models.ShareMutual,
			IsActive: true, PrecisionLevel: "exact", CreatedAt: now},
		{FriendshipID: f.ID, OwnerID: bob.ID, Direction: models.ShareMutual,
			IsActive: true, PrecisionLevel: "approximate", CreatedAt: now},
	}
	for i, ls := range shares {
		if err := p.CreateLocationShare(ls); err != nil {
			return false
		}
		lat, lon := 48.8566, 2.3522
		if i == 1 {
			lat, lon = 48.8606, 2.3376
		}
		acc := 15.0 + float64(i)*105.0
		ts := now
		ls.HasLocation = true
		ls.Latitude, ls.Longitude = lat, lon
		ls.Accuracy, ls.LastLocationUpdate = &acc, &ts
		if err := p.UpdateLocationShare(ls); err != nil {
			return false
		}
	}
	return true
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

func nullableFloat(v float64, hasLocation bool) interface{} {
	if !hasLocation {
		return nil
	}
	return v
}

func requireAffected(res sql.Result) error {
	n, err := res.RowsAffected()
	if err == nil && n == 0 {
		return ErrNotFound
	}
	return nil
}

func isUniqueViolation(err error) bool {
	// pgx surfaces unique violations through the stdlib error string; keep
	// the check driver-agnostic to avoid importing pgconn here.
	return err != nil && strings.Contains(err.Error(), "duplicate key value")
}
