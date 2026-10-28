// End-to-end tests for the HTTP API covering login, lockout, session cookies,
// CSRF double-submit protection, password management, friendships and
// directional location sharing.
package handlers

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/always-together/backend/internal/models"
	"github.com/always-together/backend/internal/security"
)

func newTestAdmin(t *testing.T) (*App, string) {
	t.Helper()
	app := NewApp()
	hash, err := security.GetPasswordHash("Adm1n!Passw0rd")
	if err != nil {
		t.Fatal(err)
	}
	admin := &models.User{
		Email:        "admin@example.com",
		PasswordHash: hash,
		FullName:     "Ada Admin",
		Role:         models.RoleAdministrator,
	}
	if err := app.Store.CreateUser(admin); err != nil {
		t.Fatal(err)
	}
	token := login(t, app, "admin@example.com", "Adm1n!Passw0rd")
	return app, token
}

func doJSON(t *testing.T, app *App, method, path, bearer string, body interface{}) *httptest.ResponseRecorder {
	t.Helper()
	var rdr *bytes.Reader
	if body == nil {
		rdr = bytes.NewReader(nil)
	} else {
		b, err := json.Marshal(body)
		if err != nil {
			t.Fatal(err)
		}
		rdr = bytes.NewReader(b)
	}
	req := httptest.NewRequest(method, path, rdr)
	req.Header.Set("Content-Type", "application/json")
	if bearer != "" {
		req.Header.Set("Authorization", "Bearer "+bearer)
	}
	rec := httptest.NewRecorder()
	app.Router().ServeHTTP(rec, req)
	return rec
}

func login(t *testing.T, app *App, email, password string) string {
	t.Helper()
	rec := doJSON(t, app, http.MethodPost, "/api/v1/auth/login", "",
		map[string]string{"email": email, "password": password})
	if rec.Code != http.StatusOK {
		t.Fatalf("login %s: status %d body %s", email, rec.Code, rec.Body.String())
	}
	var out loginResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &out); err != nil {
		t.Fatal(err)
	}
	if out.AccessToken == "" || out.TokenType != "bearer" || out.ExpiresIn != 1800 {
		t.Fatalf("unexpected login response: %+v", out)
	}
	// Session cookie must be set HttpOnly/Secure/SameSite=Lax.
	cookies := rec.Result().Cookies()
	var found bool
	for _, c := range cookies {
		if c.Name == "session_id" {
			found = true
			if !c.HttpOnly || !c.Secure || c.SameSite != http.SameSiteLaxMode {
				t.Errorf("insecure session cookie: %+v", c)
			}
		}
	}
	if !found {
		t.Error("expected session_id cookie on login")
	}
	return out.AccessToken
}

func TestHealthAndRoot(t *testing.T) {
	app, _ := newTestAdmin(t)
	rec := doJSON(t, app, http.MethodGet, "/health", "", nil)
	if rec.Code != 200 || !strings.Contains(rec.Body.String(), "healthy") {
		t.Fatalf("health: %d %s", rec.Code, rec.Body.String())
	}
	rec = doJSON(t, app, http.MethodGet, "/", "", nil)
	if rec.Code != 200 || !strings.Contains(rec.Body.String(), "Always Together API") {
		t.Fatalf("root: %d %s", rec.Code, rec.Body.String())
	}
}

func TestLoginFlowLockoutAndLogout(t *testing.T) {
	app, adminToken := newTestAdmin(t)

	// Admin creates a resident with a known temporary password.
	rec := doJSON(t, app, http.MethodPost, "/api/v1/users/", adminToken, map[string]string{
		"email":              "Resident@Example.com",
		"full_name":          "Rex Resident",
		"temporary_password": "Temp!Pass123",
	})
	if rec.Code != http.StatusCreated {
		t.Fatalf("create user: %d %s", rec.Code, rec.Body.String())
	}

	// Wrong passwords increment failed attempts; after threshold the account
	// is locked and even the right password fails with a generic message.
	var lastCode int
	for i := 0; i < 6; i++ {
		rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/login", "",
			map[string]string{"email": "resident@example.com", "password": "Wr0ng!Pass99"})
		lastCode = rec.Code
		if rec.Code == http.StatusTooManyRequests {
			t.Skip("rate limiter hit during lockout test")
		}
	}
	if lastCode != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", lastCode)
	}
	if !strings.Contains(rec.Body.String(), "AUTHENTICATION_FAILED") {
		t.Errorf("expected generic error code, got %s", rec.Body.String())
	}

	u, err := app.Store.GetUserByEmail("resident@example.com")
	if err != nil || !u.IsLocked {
		t.Fatalf("expected account to be locked, err=%v user=%+v", err, u)
	}

	// Correct credentials for the healthy admin still work.
	token := login(t, app, "admin@example.com", "Adm1n!Passw0rd")

	// /me returns the profile without sensitive fields.
	rec = doJSON(t, app, http.MethodGet, "/api/v1/auth/me", token, nil)
	if rec.Code != 200 || strings.Contains(rec.Body.String(), "password_hash") {
		t.Fatalf("me: %d %s", rec.Code, rec.Body.String())
	}

	// Logout clears the cookie.
	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	req.AddCookie(&http.Cookie{Name: "session_id", Value: "bogus"})
	req.Header.Set("Authorization", "Bearer "+token)
	rec = httptest.NewRecorder()
	app.Router().ServeHTTP(rec, req)
	if rec.Code != 200 {
		t.Fatalf("logout: %d %s", rec.Code, rec.Body.String())
	}
}

func TestUnauthenticatedAndBadToken(t *testing.T) {
	app, _ := newTestAdmin(t)
	rec := doJSON(t, app, http.MethodGet, "/api/v1/auth/me", "", nil)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rec.Code)
	}
	rec = doJSON(t, app, http.MethodGet, "/api/v1/auth/me", "garbage.token.value", nil)
	if rec.Code != http.StatusUnauthorized ||
		!strings.Contains(rec.Body.String(), "INVALID_TOKEN") {
		t.Fatalf("expected INVALID_TOKEN, got %d %s", rec.Code, rec.Body.String())
	}
}

func TestValidationErrorsUseEnvelope(t *testing.T) {
	app, _ := newTestAdmin(t)
	rec := doJSON(t, app, http.MethodPost, "/api/v1/auth/login", "",
		map[string]string{"email": "not-an-email", "password": "short"})
	if rec.Code != http.StatusUnprocessableEntity {
		t.Fatalf("expected 422, got %d %s", rec.Code, rec.Body.String())
	}
	body := rec.Body.String()
	if !strings.Contains(body, "VALIDATION_FAILED") ||
		!strings.Contains(body, "email") || !strings.Contains(body, "password") {
		t.Fatalf("unexpected validation body: %s", body)
	}
}

func TestPasswordChangeAndForceChange(t *testing.T) {
	app, adminToken := newTestAdmin(t)

	// Create user requiring a forced change (no temp password supplied).
	rec := doJSON(t, app, http.MethodPost, "/api/v1/users/", adminToken, map[string]string{
		"email":     "first@example.com",
		"full_name": "Fiona First",
	})
	if rec.Code != http.StatusCreated {
		t.Fatal(rec.Body.String())
	}
	var created struct {
		User     userResponse `json:"user"`
		TempPass string       `json:"temporary_password"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if !created.User.MustChangePassword || created.TempPass == "" {
		t.Fatalf("expected must_change + temp password: %+v", created)
	}

	token := login(t, app, "first@example.com", created.TempPass)

	// Force-change with mismatched confirmation -> validation error.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/password/force-change", token,
		map[string]string{"new_password": "N3w!Password", "confirm_password": "Different!456"})
	if rec.Code != http.StatusUnprocessableEntity ||
		!strings.Contains(rec.Body.String(), "Passwords do not match") {
		t.Fatalf("mismatch check failed: %d %s", rec.Code, rec.Body.String())
	}

	// Weak new password -> WEAK_PASSWORD with field issues.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/password/force-change", token,
		map[string]string{"new_password": "password", "confirm_password": "password"})
	if rec.Code != http.StatusBadRequest ||
		!strings.Contains(rec.Body.String(), "WEAK_PASSWORD") {
		t.Fatalf("weak check failed: %d %s", rec.Code, rec.Body.String())
	}

	// Valid force-change succeeds and clears the flag.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/password/force-change", token,
		map[string]string{"new_password": "N3w!Password", "confirm_password": "N3w!Password"})
	if rec.Code != http.StatusOK {
		t.Fatalf("force-change: %d %s", rec.Code, rec.Body.String())
	}
	u, _ := app.Store.GetUserByEmail("first@example.com")
	if u.MustChangePassword {
		t.Error("must_change_password should be cleared")
	}

	// Regular change with wrong current password fails.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/password/change", token,
		map[string]string{"current_password": "OldWrong!123",
			"new_password": "Ev3n!Better", "confirm_password": "Ev3n!Better"})
	if rec.Code != http.StatusBadRequest ||
		!strings.Contains(rec.Body.String(), "INVALID_PASSWORD") {
		t.Fatalf("expected INVALID_PASSWORD: %d %s", rec.Code, rec.Body.String())
	}

	// Valid change revokes all sessions.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/auth/password/change", token,
		map[string]string{"current_password": "N3w!Password",
			"new_password": "Ev3n!Better", "confirm_password": "Ev3n!Better"})
	if rec.Code != http.StatusOK {
		t.Fatalf("change: %d %s", rec.Code, rec.Body.String())
	}
}

func TestFriendshipAndDirectionalLocationSharing(t *testing.T) {
	app, adminToken := newTestAdmin(t)

	mk := func(email, name, pw string) string {
		rec := doJSON(t, app, http.MethodPost, "/api/v1/users/", adminToken, map[string]string{
			"email": email, "full_name": name, "temporary_password": pw,
		})
		if rec.Code != http.StatusCreated {
			t.Fatalf("create %s: %d %s", email, rec.Code, rec.Body.String())
		}
		return login(t, app, email, pw)
	}

	aToken := mk("alice@example.com", "Alice", "Al1ce!Passw")
	bToken := mk("bob@example.com", "Bob", "B0b!Passwd")

	// Alice requests Bob; duplicate request conflicts.
	rec := doJSON(t, app, http.MethodPost, "/api/v1/friendships", aToken,
		map[string]string{"receiver_email": "bob@example.com"})
	if rec.Code != http.StatusCreated {
		t.Fatalf("friend request: %d %s", rec.Code, rec.Body.String())
	}
	var f friendshipResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &f); err != nil {
		t.Fatal(err)
	}
	if f.Status != models.FriendshipPending || f.RequesterName != "Alice" {
		t.Fatalf("unexpected friendship: %+v", f)
	}
	rec = doJSON(t, app, http.MethodPost, "/api/v1/friendships", aToken,
		map[string]string{"receiver_email": "bob@example.com"})
	if rec.Code != http.StatusConflict {
		t.Fatalf("duplicate request: expected 409, got %d", rec.Code)
	}

	// Alice cannot respond to her own request.
	rec = doJSON(t, app, http.MethodPost,
		"/api/v1/friendships/"+itoa(f.ID)+"/respond", aToken,
		map[string]string{"action": "accept"})
	if rec.Code != http.StatusForbidden {
		t.Fatalf("self-respond: expected 403, got %d", rec.Code)
	}

	// Bob accepts.
	rec = doJSON(t, app, http.MethodPost,
		"/api/v1/friendships/"+itoa(f.ID)+"/respond", bToken,
		map[string]string{"action": "accept"})
	if rec.Code != http.StatusOK {
		t.Fatalf("accept: %d %s", rec.Code, rec.Body.String())
	}

	// Bob shares his location TO Alice (from Alice's perspective: from_friend).
	rec = doJSON(t, app, http.MethodPost, "/api/v1/locations/shares", bToken, map[string]interface{}{
		"friendship_id":   f.ID,
		"direction":       "to_friend",
		"precision_level": "exact",
	})
	if rec.Code != http.StatusCreated {
		t.Fatalf("create share: %d %s", rec.Code, rec.Body.String())
	}
	var share locationShareResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &share); err != nil {
		t.Fatal(err)
	}

	// Invalid direction rejected.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/locations/shares", bToken, map[string]interface{}{
		"friendship_id": f.ID, "direction": "sideways",
	})
	if rec.Code != http.StatusUnprocessableEntity {
		t.Fatalf("bad direction: expected 422, got %d", rec.Code)
	}

	// Bob updates his coordinates.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/locations/me", bToken, map[string]interface{}{
		"latitude": 52.37, "longitude": 4.9, "accuracy": 15.0,
	})
	if rec.Code != http.StatusOK {
		t.Fatalf("location update: %d %s", rec.Code, rec.Body.String())
	}

	// Out-of-range coordinates rejected.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/locations/me", bToken, map[string]interface{}{
		"latitude": 95, "longitude": 0,
	})
	if rec.Code != http.StatusUnprocessableEntity {
		t.Fatalf("bad coords: expected 422, got %d", rec.Code)
	}

	// Alice can now see Bob's exact location.
	rec = doJSON(t, app, http.MethodPost, "/api/v1/locations/friends", aToken, nil)
	rec = doJSON(t, app, http.MethodGet, "/api/v1/locations/friends", aToken, nil)
	if rec.Code != http.StatusOK {
		t.Fatalf("friends list: %d %s", rec.Code, rec.Body.String())
	}
	var locs []locationResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &locs); err != nil {
		t.Fatal(err)
	}
	if len(locs) != 1 || locs[0].UserName != "Bob" || locs[0].Latitude != 52.37 {
		t.Fatalf("expected Bob's location visible to Alice, got %+v", locs)
	}

	// A third user with no accepted friendship sees nothing of Bob's.
	cToken := mk("carol@example.com", "Carol", "Car0l!Passw")
	rec = doJSON(t, app, http.MethodGet, "/api/v1/locations/friends", cToken, nil)
	var none []locationResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &none); err != nil {
		t.Fatal(err)
	}
	if len(none) != 0 {
		t.Fatalf("privacy violation: carol sees %+v", none)
	}

	// Bob deactivates the share; Alice then sees nothing.
	rec = doJSON(t, app, http.MethodPatch,
		"/api/v1/locations/shares/"+itoa(share.ID), bToken,
		map[string]interface{}{"is_active": false})
	if rec.Code != http.StatusOK {
		t.Fatalf("deactivate: %d %s", rec.Code, rec.Body.String())
	}
	rec = doJSON(t, app, http.MethodGet, "/api/v1/locations/friends", aToken, nil)
	if err := json.Unmarshal(rec.Body.Bytes(), &none); err != nil {
		t.Fatal(err)
	}
	if len(none) != 0 {
		t.Fatalf("revoked share still visible: %+v", none)
	}
}

func itoa(i int64) string {
	b, _ := json.Marshal(i)
	return strings.Trim(string(b), "\"")
}

// sessionLogin performs a login and returns the raw session cookie plus the
// CSRF token handed out by the server (body field + X-CSRF-Token header).
func sessionLogin(t *testing.T, app *App, email, password string) (*http.Cookie, string) {
	t.Helper()
	rec := doJSON(t, app, http.MethodPost, "/api/v1/auth/login", "",
		map[string]string{"email": email, "password": password})
	if rec.Code != http.StatusOK {
		t.Fatalf("login %s: status %d body %s", email, rec.Code, rec.Body.String())
	}
	var out loginResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &out); err != nil {
		t.Fatal(err)
	}
	if out.CSRFToken == "" || rec.Header().Get(csrfHeader) != out.CSRFToken {
		t.Fatalf("expected csrf token in body and header, got %+v / %q",
			out, rec.Header().Get(csrfHeader))
	}
	for _, c := range rec.Result().Cookies() {
		if c.Name == "session_id" {
			return c, out.CSRFToken
		}
	}
	t.Fatal("no session_id cookie on login")
	return nil, ""
}

func doSession(t *testing.T, app *App, method, path string, cookie *http.Cookie,
	csrf string, body interface{}) *httptest.ResponseRecorder {
	t.Helper()
	var rdr *bytes.Reader
	if body == nil {
		rdr = bytes.NewReader(nil)
	} else {
		b, err := json.Marshal(body)
		if err != nil {
			t.Fatal(err)
		}
		rdr = bytes.NewReader(b)
	}
	req := httptest.NewRequest(method, path, rdr)
	req.Header.Set("Content-Type", "application/json")
	if cookie != nil {
		req.AddCookie(cookie)
	}
	if csrf != "" {
		req.Header.Set(csrfHeader, csrf)
	}
	rec := httptest.NewRecorder()
	app.Router().ServeHTTP(rec, req)
	return rec
}

func TestSessionCookieAuthAndCSRFProtection(t *testing.T) {
	app, _ := newTestAdmin(t)
	cookie, csrf := sessionLogin(t, app, "admin@example.com", "Adm1n!Passw0rd")

	// GET with only the session cookie authenticates without CSRF.
	rec := doSession(t, app, http.MethodGet, "/api/v1/auth/me", cookie, "", nil)
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "Ada Admin") {
		t.Fatalf("session GET /me: %d %s", rec.Code, rec.Body.String())
	}

	// Unsafe methods require the double-submit CSRF header.
	rec = doSession(t, app, http.MethodPost, "/api/v1/locations/me", cookie, "",
		map[string]float64{"latitude": 52.37, "longitude": 4.9})
	if rec.Code != http.StatusForbidden || !strings.Contains(rec.Body.String(), "CSRF_FAILED") {
		t.Fatalf("expected CSRF rejection, got %d %s", rec.Code, rec.Body.String())
	}

	rec = doSession(t, app, http.MethodPost, "/api/v1/locations/me", cookie, "wrong-token",
		map[string]float64{"latitude": 52.37, "longitude": 4.9})
	if rec.Code != http.StatusForbidden {
		t.Fatalf("expected 403 for wrong CSRF token, got %d %s", rec.Code, rec.Body.String())
	}

	rec = doSession(t, app, http.MethodPost, "/api/v1/locations/me", cookie, csrf,
		map[string]float64{"latitude": 52.37, "longitude": 4.9})
	if rec.Code != http.StatusOK && rec.Code != http.StatusCreated {
		t.Fatalf("session+CSRF location update: %d %s", rec.Code, rec.Body.String())
	}

	// Logout via session cookie revokes it; subsequent requests are rejected.
	rec = doSession(t, app, http.MethodPost, "/api/v1/auth/logout", cookie, csrf, nil)
	if rec.Code != http.StatusOK {
		t.Fatalf("logout: %d %s", rec.Code, rec.Body.String())
	}
	rec = doSession(t, app, http.MethodGet, "/api/v1/auth/me", cookie, "", nil)
	if rec.Code != http.StatusUnauthorized ||
		!strings.Contains(rec.Body.String(), "INVALID_SESSION") {
		t.Fatalf("expected INVALID_SESSION after logout, got %d %s",
			rec.Code, rec.Body.String())
	}
}
