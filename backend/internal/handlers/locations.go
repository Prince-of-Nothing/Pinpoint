// Location sharing endpoints: manage directional share permissions and
// update/view current locations.
package handlers

import (
	"math"
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"

	"github.com/always-together/backend/internal/models"
)

type locationShareCreate struct {
	FriendshipID   int64  `json:"friendship_id"`
	Direction      string `json:"direction"`
	PrecisionLevel string `json:"precision_level,omitempty"`
}

type locationShareResponse struct {
	ID                 int64                         `json:"id"`
	FriendshipID       int64                         `json:"friendship_id"`
	OwnerID            int64                         `json:"owner_id"`
	Direction          models.LocationShareDirection `json:"direction"`
	IsActive           bool                          `json:"is_active"`
	PrecisionLevel     string                        `json:"precision_level"`
	CreatedAt          time.Time                     `json:"created_at"`
	LastLocationUpdate *time.Time                    `json:"last_location_update_at"`
}

type locationUpdate struct {
	Latitude  float64    `json:"latitude"`
	Longitude float64    `json:"longitude"`
	Accuracy  *float64   `json:"accuracy,omitempty"`
	Timestamp *time.Time `json:"timestamp,omitempty"`
}

type locationResponse struct {
	UserID         int64     `json:"user_id"`
	UserName       string    `json:"user_name"`
	Latitude       float64   `json:"latitude"`
	Longitude      float64   `json:"longitude"`
	Accuracy       *float64  `json:"accuracy"`
	UpdatedAt      time.Time `json:"updated_at"`
	PrecisionLevel string    `json:"precision_level"`
}

var validDirections = map[string]bool{
	string(models.ShareToFriend):   true,
	string(models.ShareFromFriend): true,
	string(models.ShareMutual):     true,
}

var validPrecision = map[string]bool{"exact": true, "approximate": true, "city_only": true}

func toShareResponse(ls *models.LocationShare) locationShareResponse {
	return locationShareResponse{
		ID:                 ls.ID,
		FriendshipID:       ls.FriendshipID,
		OwnerID:            ls.OwnerID,
		Direction:          ls.Direction,
		IsActive:           ls.IsActive,
		PrecisionLevel:     ls.PrecisionLevel,
		CreatedAt:          ls.CreatedAt,
		LastLocationUpdate: ls.LastLocationUpdate,
	}
}

// POST /api/v1/locations/shares  (authenticated)
func (a *App) handleCreateShare(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	var body locationShareCreate
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}

	v := newValidation()
	if body.FriendshipID <= 0 {
		v.add("friendship_id", "ensure this value is greater than 0")
	}
	if !validDirections[body.Direction] {
		v.add("direction", "string does match regex '^(to_friend|from_friend|mutual)$'")
	}
	if body.PrecisionLevel == "" {
		body.PrecisionLevel = "exact"
	}
	if !validPrecision[body.PrecisionLevel] {
		v.add("precision_level", "string does match regex '^(exact|approximate|city_only)$'")
	}
	if len(v.Fields) > 0 {
		WriteError(w, r, v.err())
		return
	}

	f, err := a.Store.GetFriendship(body.FriendshipID)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "FRIENDSHIP_NOT_FOUND",
			"Friendship not found."))
		return
	}
	if f.RequesterID != caller.ID && f.ReceiverID != caller.ID {
		WriteError(w, r, errf(http.StatusForbidden, "FORBIDDEN",
			"You are not part of this friendship."))
		return
	}
	if f.Status != models.FriendshipAccepted {
		WriteError(w, r, errf(http.StatusBadRequest, "FRIENDSHIP_NOT_ACCEPTED",
			"Location sharing requires an accepted friendship."))
		return
	}

	ls := &models.LocationShare{
		FriendshipID:   f.ID,
		OwnerID:        caller.ID,
		Direction:      models.LocationShareDirection(body.Direction),
		IsActive:       true,
		PrecisionLevel: body.PrecisionLevel,
	}
	if err := a.Store.CreateLocationShare(ls); err != nil {
		WriteError(w, r, err)
		return
	}
	WriteJSON(w, http.StatusCreated, toShareResponse(ls))
}

// GET /api/v1/locations/shares  (authenticated) — my outgoing permissions.
func (a *App) handleListShares(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())
	list := a.Store.ListLocationSharesByOwner(caller.ID)
	out := make([]locationShareResponse, 0, len(list))
	for i := range list {
		out = append(out, toShareResponse(&list[i]))
	}
	WriteJSON(w, http.StatusOK, out)
}

// PATCH /api/v1/locations/shares/{id}  (authenticated, owner only) — toggle
// active state / precision level.
func (a *App) handleUpdateShare(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	id, ok := parseID(chi.URLParam(r, "shareID"))
	if !ok {
		WriteError(w, r, errf(http.StatusNotFound, "SHARE_NOT_FOUND", "Share not found."))
		return
	}
	ls, err := a.Store.GetLocationShare(id)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "SHARE_NOT_FOUND", "Share not found."))
		return
	}
	if ls.OwnerID != caller.ID {
		WriteError(w, r, errf(http.StatusForbidden, "FORBIDDEN",
			"You can only manage your own sharing permissions."))
		return
	}

	var body struct {
		IsActive       *bool   `json:"is_active,omitempty"`
		PrecisionLevel *string `json:"precision_level,omitempty"`
	}
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if body.IsActive != nil {
		ls.IsActive = *body.IsActive
	}
	if body.PrecisionLevel != nil {
		if !validPrecision[*body.PrecisionLevel] {
			v := newValidation()
			v.add("precision_level", "string does match regex '^(exact|approximate|city_only)$'")
			WriteError(w, r, v.err())
			return
		}
		ls.PrecisionLevel = *body.PrecisionLevel
	}
	if err := a.Store.UpdateLocationShare(ls); err != nil {
		WriteError(w, r, err)
		return
	}
	WriteJSON(w, http.StatusOK, toShareResponse(ls))
}

// POST /api/v1/locations/me  (authenticated) — update my current location.
func (a *App) handleUpdateMyLocation(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	var body locationUpdate
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}

	v := newValidation()
	if math.IsNaN(body.Latitude) || body.Latitude < -90 || body.Latitude > 90 {
		v.add("latitude", "ensure this value is between -90 and 90")
	}
	if math.IsNaN(body.Longitude) || body.Longitude < -180 || body.Longitude > 180 {
		v.add("longitude", "ensure this value is between -180 and 180")
	}
	if body.Accuracy != nil && (*body.Accuracy < 0 || math.IsNaN(*body.Accuracy)) {
		v.add("accuracy", "ensure this value is greater than or equal to 0")
	}
	if len(v.Fields) > 0 {
		WriteError(w, r, v.err())
		return
	}

	now := time.Now().UTC()
	updated := 0
	for _, ls := range a.Store.ListLocationSharesByOwner(caller.ID) {
		share := ls
		share.Latitude = body.Latitude
		share.Longitude = body.Longitude
		share.Accuracy = body.Accuracy
		share.HasLocation = true
		ts := now
		if body.Timestamp != nil {
			ts = body.Timestamp.UTC()
		}
		share.LastLocationUpdate = &ts
		if err := a.Store.UpdateLocationShare(&share); err == nil {
			updated++
		}
	}

	WriteJSON(w, http.StatusOK, map[string]interface{}{
		"message":        "Location updated.",
		"shares_updated": updated,
	})
}

// GET /api/v1/locations/friends  (authenticated) — friends who share with me.
func (a *App) handleListFriendLocations(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	out := []locationResponse{}
	for _, u := range a.Store.ListUsers() {
		if u.ID == caller.ID {
			continue
		}
		shares := a.Store.ListActiveSharesVisibleTo(u.ID, caller.ID)
		for i := range shares {
			ls := &shares[i]
			if !ls.HasLocation {
				continue
			}
			resp := locationResponse{
				UserID:         u.ID,
				UserName:       u.FullName,
				PrecisionLevel: ls.PrecisionLevel,
				UpdatedAt:      derefTime(ls.LastLocationUpdate),
			}
			switch ls.PrecisionLevel {
			case "exact":
				resp.Latitude = ls.Latitude
				resp.Longitude = ls.Longitude
				resp.Accuracy = ls.Accuracy
			case "approximate":
				resp.Latitude = roundTo(ls.Latitude, 2)
				resp.Longitude = roundTo(ls.Longitude, 2)
			default: // city_only
				resp.Latitude = roundTo(ls.Latitude, 1)
				resp.Longitude = roundTo(ls.Longitude, 1)
			}
			out = append(out, resp)
			break // one entry per friend
		}
	}
	WriteJSON(w, http.StatusOK, out)
}

func derefTime(t *time.Time) time.Time {
	if t == nil {
		return time.Time{}
	}
	return *t
}

func roundTo(f float64, places int) float64 {
	p := math.Pow(10, float64(places))
	return math.Round(f*p) / p
}
