// Tests for the security utilities.
package security

import (
	"strings"
	"testing"
	"time"
)

func TestPasswordHashAndVerify(t *testing.T) {
	hash, err := GetPasswordHash("SecurePassword123!")
	if err != nil {
		t.Fatalf("hash failed: %v", err)
	}
	if !VerifyPassword("SecurePassword123!", hash) {
		t.Error("expected correct password to verify")
	}
	if VerifyPassword("wrong-password", hash) {
		t.Error("expected wrong password to fail verification")
	}
}

func TestAccessAndRefreshTokens(t *testing.T) {
	data := map[string]string{"sub": "42", "email": "a@b.co", "role": "resident"}

	access, err := CreateAccessToken(data, nil)
	if err != nil {
		t.Fatalf("create access token: %v", err)
	}
	claims := DecodeToken(access)
	if claims == nil {
		t.Fatal("expected valid claims for access token")
	}
	if claims.Subject != "42" || claims.Type != "access" || claims.Role != "resident" {
		t.Errorf("unexpected claims: %+v", claims)
	}

	refresh, err := CreateRefreshToken(data)
	if err != nil {
		t.Fatalf("create refresh token: %v", err)
	}
	if c := DecodeToken(refresh); c != nil {
		t.Error("refresh tokens must not be accepted as access credentials")
	}

	if DecodeToken("not-a-jwt") != nil {
		t.Error("expected nil for garbage token")
	}

	short, err := CreateAccessToken(data, ptrDur(time.Millisecond))
	if err != nil {
		t.Fatal(err)
	}
	time.Sleep(10 * time.Millisecond)
	if DecodeToken(short) != nil {
		t.Error("expected expired token to be rejected")
	}
}

func ptrDur(d time.Duration) *time.Duration { return &d }

func TestCSRFAndSessionTokens(t *testing.T) {
	a, b := GenerateCSRFToken(), GenerateCSRFToken()
	if a == b {
		t.Error("tokens must be unique")
	}
	if !VerifyCSRFToken(a, a) {
		t.Error("identical tokens should verify")
	}
	if VerifyCSRFToken(a, b) || VerifyCSRFToken("", a) || VerifyCSRFToken(a, "") {
		t.Error("mismatched/empty tokens should not verify")
	}
	if GenerateSessionToken() == GenerateSessionToken() {
		t.Error("session tokens must be unique")
	}
}

func TestNormalizeEmail(t *testing.T) {
	if got := NormalizeEmail("  MixedCase@Example.COM "); got != "mixedcase@example.com" {
		t.Errorf("got %q", got)
	}
}

func TestIsWeakPassword(t *testing.T) {
	cases := []struct {
		pw    string
		weak  bool
		snips []string
	}{
		{"Str0ng!Pass", false, nil},
		{"short1!", true, []string{"at least 8 characters"}},
		{"alllowercase1!", true, []string{"uppercase"}},
		{"ALLUPPERCASE1!", true, []string{"lowercase"}},
		{"NoDigits!!", true, []string{"digit"}},
		{"NoSpecial1a", true, []string{"special character"}},
		{"Password", true, []string{"too common"}},
		{strings.Repeat("A1!", 50), true, []string{"128 characters"}}, // 150 chars
	}
	for _, c := range cases {
		weak, issues := IsWeakPassword(c.pw)
		if weak != c.weak {
			t.Errorf("IsWeakPassword(%q) weak=%v want %v (issues=%v)", c.pw, weak, c.weak, issues)
			continue
		}
		joined := strings.Join(issues, "|")
		for _, s := range c.snips {
			if !strings.Contains(joined, s) {
				t.Errorf("IsWeakPassword(%q): expected issue containing %q, got %v", c.pw, s, issues)
			}
		}
	}
}
