// Friendship endpoints: send, list, respond to friend requests.
package handlers

import (
	"net/http"
	"strconv"
	"time"

	"github.com/go-chi/chi/v5"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

type friendshipRequest struct {
	ReceiverEmail string `json:"receiver_email"`
}

type friendshipResponse struct {
	ID            int64                   `json:"id"`
	RequesterID   int64                   `json:"requester_id"`
	RequesterName string                  `json:"requester_name"`
	ReceiverID    int64                   `json:"receiver_id"`
	ReceiverName  string                  `json:"receiver_name"`
	Status        models.FriendshipStatus `json:"status"`
	CreatedAt     time.Time               `json:"created_at"`
	RespondedAt   *time.Time              `json:"responded_at"`
}

type friendshipAction struct {
	Action string `json:"action"` // accept|reject
}

func parseID(raw string) (int64, bool) {
	id, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || id <= 0 {
		return 0, false
	}
	return id, true
}

func (a *App) toFriendshipResponse(f *models.Friendship) (*friendshipResponse, error) {
	requester, err := a.Store.GetUserByID(f.RequesterID)
	if err != nil {
		return nil, err
	}
	receiver, err := a.Store.GetUserByID(f.ReceiverID)
	if err != nil {
		return nil, err
	}
	return &friendshipResponse{
		ID:            f.ID,
		RequesterID:   f.RequesterID,
		RequesterName: requester.FullName,
		ReceiverID:    f.ReceiverID,
		ReceiverName:  receiver.FullName,
		Status:        f.Status,
		CreatedAt:     f.CreatedAt,
		RespondedAt:   f.RespondedAt,
	}, nil
}

// POST /api/v1/friendships  (authenticated)
func (a *App) handleCreateFriendship(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	var body friendshipRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}

	v := newValidation()
	if !validEmail(body.ReceiverEmail) || len(body.ReceiverEmail) > 255 {
		v.add("receiver_email", "value is not a valid email address")
	}
	if len(v.Fields) > 0 {
		WriteError(w, r, v.err())
		return
	}

	receiver, err := a.Store.GetUserByEmail(security.NormalizeEmail(body.ReceiverEmail))
	if err != nil || !receiver.IsActive {
		WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND",
			"No active user with this email."))
		return
	}
	if receiver.ID == caller.ID {
		WriteError(w, r, errf(http.StatusBadRequest, "SELF_REQUEST",
			"You cannot send a friend request to yourself."))
		return
	}

	f := &models.Friendship{
		RequesterID: caller.ID,
		ReceiverID:  receiver.ID,
		Status:      models.FriendshipPending,
	}
	if err := a.Store.CreateFriendship(f); err != nil {
		WriteError(w, r, errf(http.StatusConflict, "FRIENDSHIP_EXISTS",
			"A pending friend request already exists between these users."))
		return
	}

	resp, err := a.toFriendshipResponse(f)
	if err != nil {
		WriteError(w, r, err)
		return
	}
	WriteJSON(w, http.StatusCreated, resp)
}

// GET /api/v1/friendships  (authenticated)
func (a *App) handleListFriendships(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())
	list := a.Store.ListFriendshipsForUser(caller.ID)
	out := make([]friendshipResponse, 0, len(list))
	for i := range list {
		resp, err := a.toFriendshipResponse(&list[i])
		if err != nil {
			continue
		}
		out = append(out, *resp)
	}
	WriteJSON(w, http.StatusOK, out)
}

// POST /api/v1/friendships/{id}/respond  (authenticated, receiver only)
func (a *App) handleRespondFriendship(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())

	id, ok := parseID(chi.URLParam(r, "friendshipID"))
	if !ok {
		WriteError(w, r, errf(http.StatusNotFound, "FRIENDSHIP_NOT_FOUND",
			"Friendship not found."))
		return
	}
	f, err := a.Store.GetFriendship(id)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "FRIENDSHIP_NOT_FOUND",
			"Friendship not found."))
		return
	}
	if f.ReceiverID != caller.ID {
		WriteError(w, r, errf(http.StatusForbidden, "FORBIDDEN",
			"Only the receiving user can respond to this request."))
		return
	}
	if f.Status != models.FriendshipPending {
		WriteError(w, r, errf(http.StatusBadRequest, "ALREADY_RESPONDED",
			"This request has already been responded to."))
		return
	}

	var body friendshipAction
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if body.Action != "accept" && body.Action != "reject" {
		v := newValidation()
		v.add("action", "string does match regex '^(accept|reject)$'")
		WriteError(w, r, v.err())
		return
	}

	now := time.Now().UTC()
	if body.Action == "accept" {
		f.Status = models.FriendshipAccepted
	} else {
		f.Status = models.FriendshipRejected
	}
	f.RespondedAt = &now
	if err := a.Store.UpdateFriendship(f); err != nil {
		WriteError(w, r, err)
		return
	}

	resp, err := a.toFriendshipResponse(f)
	if err != nil {
		WriteError(w, r, err)
		return
	}
	WriteJSON(w, http.StatusOK, resp)
}

// NOTE: DELETE /api/v1/friendships/{id} (friendship removal) is not part of
// the current API surface and is intentionally omitted.
