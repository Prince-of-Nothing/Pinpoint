// Package security implements authentication and authorization utilities:
// password hashing, JWT handling, session/CSRF token generation and
// password-strength validation.
package security

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"fmt"
	"strings"
	"time"
	"unicode"

	"github.com/golang-jwt/jwt/v5"
	"golang.org/x/crypto/bcrypt"

	"github.com/always-together/backend/internal/config"
)

const specialChars = "!@#$%^&*()_+-=[]{}|;:,.<>?"

var commonPasswords = map[string]bool{
	"password": true, "123456": true, "12345678": true, "qwerty": true,
	"abc123": true, "password123": true, "admin": true, "letmein": true,
	"welcome": true,
}

// TokenClaims are the JWT claims used for access and refresh tokens.
type TokenClaims struct {
	Type  string `json:"type"` // "access" or "refresh"
	Email string `json:"email,omitempty"`
	Role  string `json:"role,omitempty"`
	jwt.RegisteredClaims
}

// VerifyPassword checks a plain password against a bcrypt hash using a
// constant-time-safe comparison provided by the bcrypt library.
func VerifyPassword(plain, hashed string) bool {
	return bcrypt.CompareHashAndPassword([]byte(hashed), []byte(plain)) == nil
}

// GetPasswordHash returns a bcrypt hash of the password. Each call produces
// a different hash due to the random salt embedded by bcrypt.
func GetPasswordHash(password string) (string, error) {
	b, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func signingMethod(alg string) (jwt.SigningMethod, error) {
	switch strings.ToUpper(alg) {
	case "HS256", "":
		return jwt.SigningMethodHS256, nil
	case "HS384":
		return jwt.SigningMethodHS384, nil
	case "HS512":
		return jwt.SigningMethodHS512, nil
	default:
		return nil, fmt.Errorf("unsupported algorithm: %s", alg)
	}
}

func encodeToken(sub string, extra map[string]string, ttl time.Duration, tokType string) (string, error) {
	s := config.Get()
	now := time.Now().UTC()
	claims := TokenClaims{
		Type: tokType,
		RegisteredClaims: jwt.RegisteredClaims{
			Subject:   sub,
			ExpiresAt: jwt.NewNumericDate(now.Add(ttl)),
			IssuedAt:  jwt.NewNumericDate(now),
		},
	}
	for k, v := range extra {
		switch k {
		case "email":
			claims.Email = v
		case "role":
			claims.Role = v
		}
	}

	method, err := signingMethod(s.Algorithm)
	if err != nil {
		return "", err
	}
	token := jwt.NewWithClaims(method, claims)
	return token.SignedString([]byte(s.SecretKey))
}

// CreateAccessToken builds a signed JWT access token. If expiresDelta is nil
// the configured ACCESS_TOKEN_EXPIRE_MINUTES value is used. The data map may
// carry "sub", "email" and "role" keys.
func CreateAccessToken(data map[string]string, expiresDelta *time.Duration) (string, error) {
	s := config.Get()
	ttl := time.Duration(s.AccessTokenExpireMins) * time.Minute
	if expiresDelta != nil {
		ttl = *expiresDelta
	}
	sub := data["sub"]
	extra := map[string]string{"email": data["email"], "role": data["role"]}
	return encodeToken(sub, extra, ttl, "access")
}

// CreateRefreshToken builds a signed JWT refresh token with the longer
// configured expiration (REFRESH_TOKEN_EXPIRE_DAYS).
func CreateRefreshToken(data map[string]string) (string, error) {
	s := config.Get()
	ttl := time.Duration(s.RefreshTokenExpireDays) * 24 * time.Hour
	sub := data["sub"]
	extra := map[string]string{"email": data["email"], "role": data["role"]}
	return encodeToken(sub, extra, ttl, "refresh")
}

// DecodeToken validates a JWT and returns its claims. It returns nil when the
// token is invalid, expired, or not an access token.
func DecodeToken(tokenStr string) *TokenClaims {
	s := config.Get()
	method, err := signingMethod(s.Algorithm)
	if err != nil {
		return nil
	}
	var claims TokenClaims
	parsed, err := jwt.ParseWithClaims(tokenStr, &claims,
		func(t *jwt.Token) (interface{}, error) { return []byte(s.SecretKey), nil },
		jwt.WithValidMethods([]string{method.Alg()}),
	)
	if err != nil || !parsed.Valid {
		return nil
	}
	// Only access tokens grant API access.
	if claims.Type != "" && claims.Type != "access" {
		return nil
	}
	return &claims
}

func randomURLSafe(nBytes int) string {
	buf := make([]byte, nBytes)
	if _, err := rand.Read(buf); err != nil {
		panic(fmt.Sprintf("security: crypto/rand unavailable: %v", err))
	}
	return base64.RawURLEncoding.EncodeToString(buf)
}

// GenerateSessionToken returns a cryptographically secure session token
// 32 bytes, URL-safe base64 encoded).
func GenerateSessionToken() string { return randomURLSafe(32) }

// GenerateCSRFToken returns a new CSRF token.
func GenerateCSRFToken() string { return randomURLSafe(32) }

// VerifyCSRFToken compares two tokens in constant time.
func VerifyCSRFToken(token, expected string) bool {
	if token == "" || expected == "" {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(token), []byte(expected)) == 1
}

// NormalizeEmail lowercases and trims an email for consistent storage.
func NormalizeEmail(email string) string {
	return strings.ToLower(strings.TrimSpace(email))
}

// IsWeakPassword reports whether a password fails the minimum security
// requirements and lists all issues found.
func IsWeakPassword(password string) (bool, []string) {
	var issues []string

	if len(password) < 8 {
		issues = append(issues, "Password must be at least 8 characters long")
	}
	if len(password) > 128 {
		issues = append(issues, "Password must not exceed 128 characters")
	}

	var hasUpper, hasLower, hasDigit, hasSpecial bool
	for _, c := range password {
		switch {
		case unicode.IsUpper(c):
			hasUpper = true
		case unicode.IsLower(c):
			hasLower = true
		case unicode.IsDigit(c):
			hasDigit = true
		case strings.ContainsRune(specialChars, c):
			hasSpecial = true
		}
	}
	if !hasUpper {
		issues = append(issues, "Password must contain at least one uppercase letter")
	}
	if !hasLower {
		issues = append(issues, "Password must contain at least one lowercase letter")
	}
	if !hasDigit {
		issues = append(issues, "Password must contain at least one digit")
	}
	if !hasSpecial {
		issues = append(issues, "Password must contain at least one special character")
	}

	if commonPasswords[strings.ToLower(password)] {
		issues = append(issues, "Password is too common")
	}

	return len(issues) > 0, issues
}
