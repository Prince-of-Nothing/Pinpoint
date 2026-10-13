// Authentication endpoints: login, logout, password change/force-change and
// current-user retrieval.
package handlers

import (
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/always-together/backend/internal/config"
	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

// ---- Request/response schemas ----

type loginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type userResponse struct {
	ID                 int64           `json:"id"`
	Email              string          `json:"email"`
	FullName           string          `json:"full_name"`
	Role               models.UserRole `json:"role"`
	IsActive           bool            `json:"is_active"`
	MustChangePassword bool            `json:"must_change_password"`
	CreatedAt          time.Time       `json:"created_at"`
	LastLoginAt        *time.Time      `json:"last_login_at"`
}

type loginResponse struct {
	AccessToken string       `json:"access_token"`
	TokenType   string       `json:"token_type"`
	ExpiresIn   int          `json:"expires_in"`
	CSRFToken   string       `json:"csrf_token"`
	User        userResponse `json:"user"`
}

type passwordChangeRequest struct {
	CurrentPassword string `json:"current_password"`
	NewPassword     string `json:"new_password"`
	ConfirmPassword string `json:"confirm_password"`
}

type forcePasswordChangeRequest struct {
	NewPassword     string `json:"new_password"`
	ConfirmPassword string `json:"confirm_password"`
}

func toUserResponse(u *models.User) userResponse {
	return userResponse{
		ID:                 u.ID,
		Email:              u.Email,
		FullName:           u.FullName,
		Role:               u.Role,
		IsActive:           u.IsActive,
		MustChangePassword: u.MustChangePassword,
		CreatedAt:          u.CreatedAt,
		LastLoginAt:        u.LastLoginAt,
	}
}

// validateLogin enforces the field constraints for login.
func validateLogin(body *loginRequest) error {
	v := newValidation()
	if !validEmail(body.Email) {
		v.add("email", "value is not a valid email address")
	}
	if len(body.Email) > 255 {
		v.add("email", "ensure this value has at most 255 characters")
	}
	if len(body.Password) < 8 {
		v.add("password", "ensure this value has at least 8 characters")
	}
	if len(body.Password) > 128 {
		v.add("password", "ensure this value has at most 128 characters")
	}
	if len(v.Fields) > 0 {
		return v.err()
	}
	return nil
}

func validEmail(e string) bool {
	e = strings.TrimSpace(e)
	at := strings.Index(e, "@")
	if at <= 0 || at == len(e)-1 || strings.ContainsAny(e, " \t") {
		return false
	}
	dot := strings.LastIndex(e[at:], ".")
	return dot > 0 && dot < len(e)-at-1
}

// validatePasswordFields enforces min/max length + confirmation match,
// returning VALIDATION_FAILED or WEAK_PASSWORD style errors via helpers below.
func validatePasswordPair(cur, newPw, confirm string, hasCurrent bool) error {
	v := newValidation()
	checkLen := func(field, val string) {
		if len(val) < 8 {
			v.add(field, "ensure this value has at least 8 characters")
		}
		if len(val) > 128 {
			v.add(field, "ensure this value has at most 128 characters")
		}
	}
	if hasCurrent {
		checkLen("current_password", cur)
	}
	checkLen("new_password", newPw)
	checkLen("confirm_password", confirm)
	if len(v.Fields) == 0 && newPw != confirm {
		v.add("confirm_password", "Passwords do not match")
	}
	if len(v.Fields) > 0 {
		return v.err()
	}
	return nil
}

// POST /api/v1/auth/login
func (a *App) handleLogin(w http.ResponseWriter, r *http.Request) {
	s := config.Get()

	var body loginRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if err := validateLogin(&body); err != nil {
		WriteError(w, r, err)
		return
	}

	clientIP := ClientIP(r)

	// Rate limiting by IP (fixed window; Redis in production).
	if !a.RateLimiter.Allow(rateLimitKey(clientIP)) {
		WriteError(w, r, errf(http.StatusTooManyRequests, "RATE_LIMITED",
			"Too many login attempts. Please try again later."))
		return
	}

	normalizedEmail := security.NormalizeEmail(body.Email)

	// Account lockout check.
	if a.isLockedOut(normalizedEmail) {
		a.Store.RecordLoginAttempt(normalizedEmail, clientIP, false)
		WriteError(w, r, errf(http.StatusUnauthorized, "AUTHENTICATION_FAILED",
			"Invalid credentials."))
		return
	}

	user, err := a.Store.GetUserByEmail(normalizedEmail)
	authFail := func() {
		if user != nil {
			failedAttempts := user.FailedLoginAttempts + 1
			user.FailedLoginAttempts = failedAttempts
			if failedAttempts >= s.LockoutThreshold {
				now := time.Now().UTC()
				user.IsLocked = true
				user.LockedAt = &now
				a.setLockout(normalizedEmail)
			}
			_ = a.Store.UpdateUser(user)
		}
		a.Store.RecordLoginAttempt(normalizedEmail, clientIP, false)
		WriteError(w, r, errf(http.StatusUnauthorized, "AUTHENTICATION_FAILED",
			"Invalid credentials."))
	}

	// Generic error for unknown/inactive/locked users.
	if err != nil || !user.IsActive || user.IsLocked {
		authFail()
		return
	}

	if !security.VerifyPassword(body.Password, user.PasswordHash) {
		authFail()
		return
	}

	// Successful login - reset failed attempts.
	user.FailedLoginAttempts = 0
	now := time.Now().UTC()
	user.LastLoginAt = &now
	if err := a.Store.UpdateUser(user); err != nil {
		WriteError(w, r, err)
		return
	}

	// Create server-side session.
	sessionToken := security.GenerateSessionToken()
	expiresAt := now.Add(time.Duration(s.SessionMaxAgeSeconds) * time.Second)
	sess := &models.Session{
		SessionToken: sessionToken,
		UserID:       user.ID,
		IPAddress:    clientIP,
		UserAgent:    r.Header.Get("User-Agent"),
		ExpiresAt:    expiresAt,
	}
	if err := a.Store.CreateSession(sess); err != nil {
		WriteError(w, r, err)
		return
	}

	// JWT access token.
	accessToken, err := security.CreateAccessToken(map[string]string{
		"sub":   strconv.FormatInt(user.ID, 10),
		"email": user.Email,
		"role":  string(user.Role),
	}, nil)
	if err != nil {
		WriteError(w, r, err)
		return
	}

	http.SetCookie(w, sessionCookie(sessionToken, s.SessionMaxAgeSeconds))
	w.Header().Set(csrfHeader, sess.CSRFToken)
	http.SetCookie(w, csrfCookie(sess.CSRFToken, s.SessionMaxAgeSeconds))

	WriteJSON(w, http.StatusOK, loginResponse{
		AccessToken: accessToken,
		TokenType:   "bearer",
		ExpiresIn:   s.AccessTokenExpireMins * 60,
		CSRFToken:   sess.CSRFToken,
		User:        toUserResponse(user),
	})
}

// isLockedOut reports whether an account should be rejected due to lockout,
// honouring LOCKOUT_DURATION_MINUTES for auto-unlock.
func (a *App) isLockedOut(email string) bool {
	key := lockoutKey(email)
	a.lockMu.Lock()
	defer a.lockMu.Unlock()

	until, ok := a.lockouts[key]
	if !ok {
		return false
	}
	if time.Now().UTC().After(until) {
		delete(a.lockouts, key)
		return false
	}
	return true
}

// setLockout records a lockout for an email for LOCKOUT_DURATION_MINUTES.
func (a *App) setLockout(email string) {
	s := config.Get()
	a.lockMu.Lock()
	defer a.lockMu.Unlock()
	a.lockouts[lockoutKey(email)] = time.Now().UTC().
		Add(time.Duration(s.LockoutDurationMins) * time.Minute)
}

// clearLockout removes any pending lockout for an email (used by the admin
// account-unlock action in weba).
func (a *App) clearLockout(email string) {
	a.lockMu.Lock()
	defer a.lockMu.Unlock()
	delete(a.lockouts, lockoutKey(email))
}

// POST /api/v1/auth/logout
func (a *App) handleLogout(w http.ResponseWriter, r *http.Request) {
	s := config.Get()

	var message string
	if c, err := r.Cookie(s.SessionCookieName); err == nil && c.Value != "" {
		a.Store.RevokeSession(c.Value)
		message = "Successfully logged out"
	} else if bearer := bearerToken(r); bearer != "" {
		// Bearer JWTs are stateless; also revoke any session belonging to the
		// token's subject so logout fully invalidates server-side state.
		if claims := security.DecodeToken(bearer); claims != nil {
			if id, perr := strconv.ParseInt(claims.Subject, 10, 64); perr == nil {
				a.Store.RevokeAllSessionsForUser(id)
			}
		}
		message = "Successfully logged out"
	} else {
		message = "No active session"
	}

	http.SetCookie(w, clearCookie())
	http.SetCookie(w, csrfCookie("", -1))

	WriteJSON(w, http.StatusOK, map[string]string{"message": message})
}

// POST /api/v1/auth/password/change
func (a *App) handleChangePassword(w http.ResponseWriter, r *http.Request) {
	user := userFromContext(r.Context())

	var body passwordChangeRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if err := validatePasswordPair(body.CurrentPassword, body.NewPassword,
		body.ConfirmPassword, true); err != nil {
		WriteError(w, r, err)
		return
	}

	if !security.VerifyPassword(body.CurrentPassword, user.PasswordHash) {
		WriteError(w, r, errf(http.StatusBadRequest, "INVALID_PASSWORD",
			"Current password is incorrect."))
		return
	}

	if weak, issues := security.IsWeakPassword(body.NewPassword); weak {
		WriteError(w, r, &APIError{
			Status:   http.StatusBadRequest,
			Code:     "WEAK_PASSWORD",
			Message:  "Password does not meet security requirements.",
			FieldErr: map[string][]string{"new_password": issues},
		})
		return
	}

	hash, err := security.GetPasswordHash(body.NewPassword)
	if err != nil {
		WriteError(w, r, err)
		return
	}
	user.PasswordHash = hash
	user.MustChangePassword = false
	if err := a.Store.UpdateUser(user); err != nil {
		WriteError(w, r, err)
		return
	}

	// Invalidate all sessions on password change.
	a.Store.RevokeAllSessionsForUser(user.ID)

	WriteJSON(w, http.StatusOK, map[string]string{
		"message": "Password changed successfully. Please log in again.",
	})
}

// POST /api/v1/auth/password/force-change
func (a *App) handleForceChangePassword(w http.ResponseWriter, r *http.Request) {
	user := userFromContext(r.Context())

	var body forcePasswordChangeRequest
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	if err := validatePasswordPair("", body.NewPassword, body.ConfirmPassword,
		false); err != nil {
		WriteError(w, r, err)
		return
	}

	if !user.MustChangePassword {
		WriteError(w, r, errf(http.StatusBadRequest, "NOT_REQUIRED",
			"Password change is not required."))
		return
	}

	if weak, issues := security.IsWeakPassword(body.NewPassword); weak {
		WriteError(w, r, &APIError{
			Status:   http.StatusBadRequest,
			Code:     "WEAK_PASSWORD",
			Message:  "Password does not meet security requirements.",
			FieldErr: map[string][]string{"new_password": issues},
		})
		return
	}

	hash, err := security.GetPasswordHash(body.NewPassword)
	if err != nil {
		WriteError(w, r, err)
		return
	}
	user.PasswordHash = hash
	user.MustChangePassword = false
	if err := a.Store.UpdateUser(user); err != nil {
		WriteError(w, r, err)
		return
	}

	WriteJSON(w, http.StatusOK, map[string]string{
		"message": "Password changed successfully.",
	})
}

// GET /api/v1/auth/me
func (a *App) handleMe(w http.ResponseWriter, r *http.Request) {
	user := userFromContext(r.Context())
	WriteJSON(w, http.StatusOK, toUserResponse(user))
}

// handleSessionRefresh POST /api/v1/auth/session
//
// Cookie-session bootstrap for web clients: requireAuth has already validated
// the session cookie and refreshed its idle timer (and re-mirrored the CSRF
// cookie). This handler returns the current user so the SPA / weba / webk can
// restore their state after a page reload without re-logging in.
func (a *App) handleSessionRefresh(w http.ResponseWriter, r *http.Request) {
	user := userFromContext(r.Context())
	WriteJSON(w, http.StatusOK, map[string]interface{}{
		"user": toUserResponse(user),
	})
}
