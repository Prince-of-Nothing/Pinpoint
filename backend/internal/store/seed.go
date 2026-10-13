package store

import (
	"time"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

// SeedDemoData populates the store with an administrator and two accepted
// friends with mutual location sharing enabled. It is idempotent: it is a
// no-op when users already exist. Returns true when data was seeded so the
// caller can log the demo credentials.
func (s *Store) SeedDemoData() bool {
	s.mu.Lock()
	if len(s.users) > 0 {
		s.mu.Unlock()
		return false
	}
	s.mu.Unlock()

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
		if err := s.CreateUser(u); err != nil {
			return false
		}
	}

	now := time.Now().UTC()
	f := &models.Friendship{
		RequesterID: alice.ID, ReceiverID: bob.ID,
		Status: models.FriendshipAccepted, CreatedAt: now, RespondedAt: &now,
	}
	if err := s.CreateFriendship(f); err != nil {
		return false
	}

	shares := []*models.LocationShare{
		{FriendshipID: f.ID, OwnerID: alice.ID, Direction: models.ShareMutual,
			IsActive: true, PrecisionLevel: "exact", CreatedAt: now},
		{FriendshipID: f.ID, OwnerID: bob.ID, Direction: models.ShareMutual,
			IsActive: true, PrecisionLevel: "approximate", CreatedAt: now},
	}
	for i, ls := range shares {
		if err := s.CreateLocationShare(ls); err != nil {
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
		if err := s.UpdateLocationShare(ls); err != nil {
			return false
		}
	}
	return true
}
