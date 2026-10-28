// Repository conformance suite (T01): the same behavioral contract run
// against every configured driver.
//
// The memory driver always runs. The postgres driver runs when
// TEST_DATABASE_URL is set and reachable (CI provides one); it is skipped
// otherwise so local `go test ./...` needs no infrastructure.
//
// IMPORTANT: postgres tests share one database and truncate tables per run,
// so TEST_DATABASE_URL must point at a DEDICATED test database.
package store

import (
	"os"
	"testing"
	"time"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

// newDrivers returns every repository implementation under test.
func newDrivers(t *testing.T) map[string]Repository {
	t.Helper()
	drivers := map[string]Repository{
		DriverMemory: New(),
	}
	if url := os.Getenv("TEST_DATABASE_URL"); url != "" {
		pg, err := OpenPostgres(url)
		if err != nil {
			t.Fatalf("postgres driver configured but unusable: %v", err)
		}
		t.Cleanup(func() { _ = pg.Close() })
		drivers[DriverPostgres] = pg
	} else {
		t.Log("TEST_DATABASE_URL not set; postgres driver skipped")
	}
	return drivers
}

// reset wipes all tables so each driver starts from a clean slate.
func reset(t *testing.T, repo Repository) {
	t.Helper()
	pg, ok := repo.(*Postgres)
	if !ok {
		return // memory driver is fresh per test
	}
	for _, table := range []string{
		"location_shares", "friendships", "sessions", "login_attempts", "users",
	} {
		if _, err := pg.db.Exec("TRUNCATE TABLE " + table + " RESTART IDENTITY CASCADE"); err != nil {
			t.Fatalf("truncate %s: %v", table, err)
		}
	}
}

// forEachDriver runs fn against each driver with subtests named by driver.
func forEachDriver(t *testing.T, fn func(t *testing.T, repo Repository)) {
	t.Helper()
	for name, repo := range newDrivers(t) {
		t.Run(name, func(t *testing.T) {
			reset(t, repo)
			fn(t, repo)
		})
	}
}

func mustUser(t *testing.T, repo Repository, email string) *models.User {
	t.Helper()
	hash, err := security.GetPasswordHash("Conformance1 Pass!")
	if err != nil {
		t.Fatalf("hash: %v", err)
	}
	u := &models.User{Email: email, PasswordHash: hash, FullName: "Test"}
	if err := repo.CreateUser(u); err != nil {
		t.Fatalf("create user %s: %v", email, err)
	}
	return u
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

func TestConformanceUserLifecycle(t *testing.T) {
	forEachDriver(t, func(t *testing.T, repo Repository) {
		u := mustUser(t, repo, "Alice@Example.com")

		if u.ID == 0 {
			t.Fatal("expected assigned id")
		}
		if u.EmailNorm != "alice@example.com" {
			t.Fatalf("normalized email = %q", u.EmailNorm)
		}
		if !u.IsActive || u.Role != models.RoleResident {
			t.Fatalf("defaults not applied: active=%v role=%q", u.IsActive, u.Role)
		}

		// Duplicate email (any casing) rejected.
		dup := &models.User{Email: "ALICE@example.com", PasswordHash: "x"}
		if err := repo.CreateUser(dup); err == nil {
			t.Fatal("expected duplicate email error")
		}

		// Lookup by id and normalized email.
		byID, err := repo.GetUserByID(u.ID)
		if err != nil || byID.Email != "Alice@Example.com" {
			t.Fatalf("GetUserByID: %v %+v", err, byID)
		}
		if _, err := repo.GetUserByEmail("  ALICE@example.com "); err != nil {
			t.Fatalf("GetUserByEmail normalized lookup: %v", err)
		}
		if _, err := repo.GetUserByID(999999); err != ErrNotFound {
			t.Fatalf("missing user should be ErrNotFound, got %v", err)
		}

		// Update preserves creation time and refreshes normalized email.
		created := u.CreatedAt
		u.FullName = "Alice Renamed"
		u.Email = "alice_new@example.com"
		if err := repo.UpdateUser(u); err != nil {
			t.Fatalf("update: %v", err)
		}
		got, err := repo.GetUserByEmail("alice_new@example.com")
		if err != nil || got.FullName != "Alice Renamed" {
			t.Fatalf("after update: %v %+v", err, got)
		}
		if !got.CreatedAt.Equal(created) {
			t.Fatalf("created_at changed: %v -> %v", created, got.CreatedAt)
		}
		if _, err := repo.GetUserByEmail("alice@example.com"); err != ErrNotFound {
			t.Fatal("old email index should be gone after update")
		}

		// List contains exactly one user.
		if n := len(repo.ListUsers()); n != 1 {
			t.Fatalf("ListUsers = %d, want 1", n)
		}

		// Deactivating revokes sessions (checked in session tests below).
		if err := repo.SetUserActive(u.ID, false); err != nil {
			t.Fatalf("deactivate: %v", err)
		}
		got, _ = repo.GetUserByID(u.ID)
		if got.IsActive {
			t.Fatal("user should be inactive")
		}
		if err := repo.UnlockUser(u.ID); err != nil {
			t.Fatalf("unlock: %v", err)
		}
	})
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

func TestConformanceSessions(t *testing.T) {
	forEachDriver(t, func(t *testing.T, repo Repository) {
		u := mustUser(t, repo, "sess@example.com")
		other := mustUser(t, repo, "other@example.com")

		sess := &models.Session{
			SessionToken: "tok-1", UserID: u.ID,
			ExpiresAt: time.Now().UTC().Add(time.Hour),
		}
		if err := repo.CreateSession(sess); err != nil {
			t.Fatalf("create session: %v", err)
		}
		if sess.CSRFToken == "" {
			t.Fatal("csrf token should be auto-generated")
		}

		// Unknown user rejected.
		bad := &models.Session{SessionToken: "tok-bad", UserID: 999999,
			ExpiresAt: time.Now().UTC().Add(time.Hour)}
		if err := repo.CreateSession(bad); err == nil {
			t.Fatal("expected error for unknown session user")
		}

		// Retrieval by token.
		got, err := repo.GetSession("tok-1")
		if err != nil || got.UserID != u.ID {
			t.Fatalf("GetSession: %v %+v", err, got)
		}
		if _, err := repo.GetSession("nope"); err != ErrNotFound {
			t.Fatalf("missing session should be ErrNotFound, got %v", err)
		}

		// Touch updates activity. (Small sleep: coarse clock resolution can
		// make consecutive time.Now() calls return identical values.)
		time.Sleep(2 * time.Millisecond)
		repo.TouchSession("tok-1")
		got, _ = repo.GetSession("tok-1")
		if !got.LastActivityAt.After(sess.CreatedAt) {
			t.Fatal("TouchSession should advance last_activity_at")
		}

		// Revoke single, then all for user.
		repo.RevokeSession("tok-1")
		got, _ = repo.GetSession("tok-1")
		if got.RevokedAt == nil {
			t.Fatal("session should be revoked")
		}

		sess2 := &models.Session{SessionToken: "tok-2", UserID: u.ID,
			ExpiresAt: time.Now().UTC().Add(time.Hour)}
		if err := repo.CreateSession(sess2); err != nil {
			t.Fatalf("create session 2: %v", err)
		}
		sess3 := &models.Session{SessionToken: "tok-3", UserID: other.ID,
			ExpiresAt: time.Now().UTC().Add(time.Hour)}
		if err := repo.CreateSession(sess3); err != nil {
			t.Fatalf("create session 3: %v", err)
		}
		repo.RevokeAllSessionsForUser(u.ID)
		if got, _ = repo.GetSession("tok-2"); got.RevokedAt == nil {
			t.Fatal("tok-2 should be revoked by RevokeAllSessionsForUser")
		}
		if got, _ = repo.GetSession("tok-3"); got.RevokedAt != nil {
			t.Fatal("other user's session must not be revoked")
		}

		// Deactivation revokes active sessions.
		sess4 := &models.Session{SessionToken: "tok-4", UserID: other.ID,
			ExpiresAt: time.Now().UTC().Add(time.Hour)}
		if err := repo.CreateSession(sess4); err != nil {
			t.Fatalf("create session 4: %v", err)
		}
		if err := repo.SetUserActive(other.ID, false); err != nil {
			t.Fatalf("deactivate: %v", err)
		}
		if got, _ = repo.GetSession("tok-4"); got.RevokedAt == nil {
			t.Fatal("deactivation must revoke active sessions")
		}
	})
}

// ---------------------------------------------------------------------------
// Login attempts
// ---------------------------------------------------------------------------

func TestConformanceLoginAttempts(t *testing.T) {
	forEachDriver(t, func(t *testing.T, repo Repository) {
		repo.RecordLoginAttempt("victim@example.com", "10.0.0.1", false)
		repo.RecordLoginAttempt("Victim@example.com", "10.0.0.2", false)
		repo.RecordLoginAttempt("victim@example.com", "10.0.0.3", true)
		repo.RecordLoginAttempt("other@example.com", "10.0.0.4", false)

		if n := repo.RecentFailedAttempts("victim@example.com", time.Hour); n != 2 {
			t.Fatalf("failed attempts = %d, want 2 (case-insensitive, failures only)", n)
		}
		// Tiny window + short sleep: attempts fall outside it even on coarse
		// clocks (Windows time.Now resolution can equal the attempt stamps).
		time.Sleep(5 * time.Millisecond)
		if n := repo.RecentFailedAttempts("victim@example.com", time.Microsecond); n != 0 {
			t.Fatalf("expired window should count 0, got %d", n)
		}
	})
}

// ---------------------------------------------------------------------------
// Friendships
// ---------------------------------------------------------------------------

func TestConformanceFriendships(t *testing.T) {
	forEachDriver(t, func(t *testing.T, repo Repository) {
		a := mustUser(t, repo, "a@example.com")
		b := mustUser(t, repo, "b@example.com")

		f := &models.Friendship{RequesterID: a.ID, ReceiverID: b.ID}
		if err := repo.CreateFriendship(f); err != nil {
			t.Fatalf("create friendship: %v", err)
		}
		if f.Status != models.FriendshipPending {
			t.Fatalf("default status = %q", f.Status)
		}

		// Duplicate pending in either direction rejected.
		dup := &models.Friendship{RequesterID: a.ID, ReceiverID: b.ID}
		if err := repo.CreateFriendship(dup); err == nil {
			t.Fatal("same-direction duplicate should fail")
		}
		revDup := &models.Friendship{RequesterID: b.ID, ReceiverID: a.ID}
		if err := repo.CreateFriendship(revDup); err == nil {
			t.Fatal("reverse-direction duplicate should fail")
		}

		// Accept and persist.
		f.Status = models.FriendshipAccepted
		now := time.Now().UTC()
		f.RespondedAt = &now
		if err := repo.UpdateFriendship(f); err != nil {
			t.Fatalf("update friendship: %v", err)
		}
		got, err := repo.GetFriendship(f.ID)
		if err != nil || got.Status != models.FriendshipAccepted {
			t.Fatalf("after accept: %v %+v", err, got)
		}
		if _, err := repo.GetFriendship(999999); err != ErrNotFound {
			t.Fatalf("missing friendship should be ErrNotFound, got %v", err)
		}

		// Both parties see it; a stranger does not.
		if n := len(repo.ListFriendshipsForUser(a.ID)); n != 1 {
			t.Fatalf("requester sees %d, want 1", n)
		}
		if n := len(repo.ListFriendshipsForUser(b.ID)); n != 1 {
			t.Fatalf("receiver sees %d, want 1", n)
		}
	})
}

// ---------------------------------------------------------------------------
// Location shares
// ---------------------------------------------------------------------------

func TestConformanceLocationShares(t *testing.T) {
	forEachDriver(t, func(t *testing.T, repo Repository) {
		owner := mustUser(t, repo, "owner@example.com")
		viewer := mustUser(t, repo, "viewer@example.com")
		stranger := mustUser(t, repo, "stranger@example.com")

		f := &models.Friendship{RequesterID: owner.ID, ReceiverID: viewer.ID,
			Status: models.FriendshipAccepted}
		if err := repo.CreateFriendship(f); err != nil {
			t.Fatalf("create friendship: %v", err)
		}

		// Share must reference an existing friendship.
		orphan := &models.LocationShare{FriendshipID: 999999, OwnerID: owner.ID}
		if err := repo.CreateLocationShare(orphan); err == nil {
			t.Fatal("orphan share should fail")
		}

		acc := 12.5
		ls := &models.LocationShare{
			FriendshipID: f.ID, OwnerID: owner.ID, Direction: models.ShareToFriend,
			IsActive: true, PrecisionLevel: "approximate",
			HasLocation: true, Latitude: 48.85, Longitude: 2.35, Accuracy: &acc,
		}
		if err := repo.CreateLocationShare(ls); err != nil {
			t.Fatalf("create share: %v", err)
		}

		// Visible to the accepted friend, never to a stranger.
		visible := repo.ListActiveSharesVisibleTo(owner.ID, viewer.ID)
		if len(visible) != 1 {
			t.Fatalf("viewer sees %d shares, want 1", len(visible))
		}
		if visible[0].Latitude != 48.85 || visible[0].Longitude != 2.35 {
			t.Fatalf("coordinates not preserved: %+v", visible[0])
		}
		if n := len(repo.ListActiveSharesVisibleTo(owner.ID, stranger.ID)); n != 0 {
			t.Fatalf("stranger sees %d shares, want 0", n)
		}

		// Inactive shares disappear from visibility.
		ls.IsActive = false
		if err := repo.UpdateLocationShare(ls); err != nil {
			t.Fatalf("deactivate share: %v", err)
		}
		if n := len(repo.ListActiveSharesVisibleTo(owner.ID, viewer.ID)); n != 0 {
			t.Fatalf("inactive share still visible (%d)", n)
		}

		// from_friend direction is never visible via ListActiveSharesVisibleTo.
		ls.IsActive = true
		ls.Direction = models.ShareFromFriend
		if err := repo.UpdateLocationShare(ls); err != nil {
			t.Fatalf("flip direction: %v", err)
		}
		if n := len(repo.ListActiveSharesVisibleTo(owner.ID, viewer.ID)); n != 0 {
			t.Fatalf("from_friend share should not be visible (%d)", n)
		}

		// Owner listing returns the share; location round-trips.
		owned := repo.ListLocationSharesByOwner(owner.ID)
		if len(owned) != 1 || !owned[0].HasLocation || owned[0].Accuracy == nil ||
			*owned[0].Accuracy != acc {
			t.Fatalf("owner list round-trip failed: %+v", owned)
		}
		if n := len(repo.ListLocationSharesByOwner(viewer.ID)); n != 0 {
			t.Fatalf("viewer owns %d shares, want 0", n)
		}

		// Location-less shares store NULLs, not zeros.
		plain := &models.LocationShare{FriendshipID: f.ID, OwnerID: owner.ID,
			Direction: models.ShareToFriend, IsActive: true, PrecisionLevel: "exact"}
		if err := repo.CreateLocationShare(plain); err != nil {
			t.Fatalf("create plain share: %v", err)
		}
		got, err := repo.GetLocationShare(plain.ID)
		if err != nil || got.HasLocation {
			t.Fatalf("plain share: %v %+v", err, got)
		}
		if _, err := repo.GetLocationShare(999999); err != ErrNotFound {
			t.Fatalf("missing share should be ErrNotFound, got %v", err)
		}
	})
}
