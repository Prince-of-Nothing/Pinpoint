// User management endpoints (admin-only user provisioning).
package handlers

import (
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

type userCreateRequest struct {
	Email             string          `json:"email"`
	FullName          string          `json:"full_name"`
	Role              models.UserRole `json:"role,omitempty"`
	TemporaryPassword string          `json:"temporary_password,omitempty"`
}

type userUpdateRequest struct {
	FullName *string `json:"full_name,omitempty"`
}

func validateUserCreate(body *userCreateRequest) error {
	v := newValidation()
	if !validEmail(body.Email) || len(body.Email) > 255 {
		v.add("email", "value is not a valid email address")
	}
	name := strings.TrimSpace(body.FullName)
	if name == "" {
		v.add("full_name", "ensure this value has at least 1 characters")
	}
	if len(name) > 100 {
		v.add("full_name", "ensure this value has at most 100 characters")
	}
	if body.Role != "" && body.Role != models.RoleResident && body.Role != models.RoleAdministrator {
		v.add("role", "value must be 'resident' or 'administrator'")
	}
	if body.TemporaryPassword != "" {
		if len(body.TemporaryPassword) < 8 {
			v.add("temporary_password", "ensure this value has at least 8 characters")
		}
		if len(body.TemporaryPassword) > 128 {
			v.add("temporary_password", "ensure this value has at most 128 characters")
		}
	}
	if len(v.Fields) > 0 {
		return v.err()
	}
	return nil
}

// requireAdmin wraps a handler, allowing only administrator role bearers.
func (a *App) requireAdmin(next http.Handler) http.Handler {
	return a.requireAuth(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		user := userFromContext(r.Context())
		if user.Role != models.RoleAdministrator {
			WriteError(w, r, errf(http.StatusForbidden, "FORBIDDEN",
				"Administrator privileges required."))
			return
		}
		next.ServeHTTP(w, r)
	}))
}

// POST /api/v1/users  (admin only)
func (a *App) handleCreateUser(w http.ResponseWriter, r *http.Request) {
	var body userCreateRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if err := validateUserCreate(&body); err != nil {
		WriteError(w, r, err)
		return
	}

	tempPassword := body.TemporaryPassword
	mustChange := true
	if tempPassword == "" {
		// Generate a secure temporary password for first-time login.
		tempPassword = security.GenerateSessionToken()
	}

	hash, err := security.GetPasswordHash(tempPassword)
	if err != nil {
		WriteError(w, r, err)
		return
	}

	role := body.Role
	if role == "" {
		role = models.RoleResident
	}

	user := &models.User{
		Email:              strings.TrimSpace(body.Email),
		PasswordHash:       hash,
		FullName:           strings.TrimSpace(body.FullName),
		Role:               role,
		MustChangePassword: mustChange,
	}
	if err := a.Store.CreateUser(user); err != nil {
		if strings.Contains(err.Error(), "already registered") {
			WriteError(w, r, errf(http.StatusConflict, "EMAIL_EXISTS",
				"A user with this email already exists."))
			return
		}
		WriteError(w, r, err)
		return
	}

	resp := map[string]interface{}{"user": toUserResponse(user)}
	if body.TemporaryPassword == "" {
		// Only returned once; must be delivered out-of-band in production.
		resp["temporary_password"] = tempPassword
	}
	WriteJSON(w, http.StatusCreated, resp)
}

// GET /api/v1/users  (authenticated) — list directory entries.
func (a *App) handleListUsers(w http.ResponseWriter, r *http.Request) {
	users := a.Store.ListUsers()
	out := make([]userResponse, 0, len(users))
	for i := range users {
		out = append(out, toUserResponse(&users[i]))
	}
	WriteJSON(w, http.StatusOK, out)
}

// GET /api/v1/users/{id}  (authenticated)
func (a *App) handleGetUser(w http.ResponseWriter, r *http.Request) {
	id, ok := parseID(chi.URLParam(r, "userID"))
	if !ok {
		WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND", "User not found."))
		return
	}
	user, err := a.Store.GetUserByID(id)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND", "User not found."))
		return
	}
	WriteJSON(w, http.StatusOK, toUserResponse(user))
}

// PATCH /api/v1/users/{id}  (self or admin)
func (a *App) handleUpdateUser(w http.ResponseWriter, r *http.Request) {
	caller := userFromContext(r.Context())
	id, ok := parseID(chi.URLParam(r, "userID"))
	if !ok {
		WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND", "User not found."))
		return
	}
	if caller.ID != id && caller.Role != models.RoleAdministrator {
		WriteError(w, r, errf(http.StatusForbidden, "FORBIDDEN",
			"You can only update your own profile."))
		return
	}

	user, err := a.Store.GetUserByID(id)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "USER_NOT_FOUND", "User not found."))
		return
	}

	var body userUpdateRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if body.FullName != nil {
		name := strings.TrimSpace(*body.FullName)
		if name == "" || len(name) > 100 {
			v := newValidation()
			v.add("full_name", "ensure this value is between 1 and 100 characters")
			WriteError(w, r, v.err())
			return
		}
		user.FullName = name
	}
	if err := a.Store.UpdateUser(user); err != nil {
		WriteError(w, r, err)
		return
	}
	WriteJSON(w, http.StatusOK, toUserResponse(user))
}
