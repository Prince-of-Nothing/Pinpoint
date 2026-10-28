// Package handlers implements the HTTP layer of the Always Together API:
// routing, request validation, JSON error responses, CORS and authentication
// middleware.
package handlers

import (
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strings"

	"github.com/always-together/backend/internal/config"
)

// Version is the API version reported by /health and /.
const Version = "0.1.0"

// ErrorDetail is the standard machine-readable error payload used across the
// API (machine-readable error envelope with optional field errors).
type ErrorDetail struct {
	Code    string              `json:"code"`
	Message string              `json:"message"`
	Fields  map[string][]string `json:"fields,omitempty"`
}

type errorEnvelope struct {
	Error ErrorDetail `json:"error"`
}

// APIError is an HTTP error carrying a stable error code and optional field
// level validation messages.
type APIError struct {
	Status   int
	Code     string
	Message  string
	FieldErr map[string][]string
}

func (e *APIError) Error() string { return e.Code + ": " + e.Message }

func errf(status int, code, msg string) *APIError {
	return &APIError{Status: status, Code: code, Message: msg}
}

// Validation produces the VALIDATION_FAILED envelope used for malformed or
// invalid request bodies.
type Validation struct{ Fields map[string][]string }

func newValidation() *Validation { return &Validation{Fields: map[string][]string{}} }

func (v *Validation) add(field, msg string) { v.Fields[field] = append(v.Fields[field], msg) }

func (v *Validation) err() *APIError {
	return &APIError{
		Status:   http.StatusUnprocessableEntity,
		Code:     "VALIDATION_FAILED",
		Message:  "Validation failed.",
		FieldErr: v.Fields,
	}
}

// WriteJSON serializes v as a JSON response with the given status code.
func WriteJSON(w http.ResponseWriter, status int, v interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

// WriteError emits the standard error envelope. Unexpected errors never leak
// internal details outside debug mode (generic exception handler parity).
func WriteError(w http.ResponseWriter, r *http.Request, err error) {
	s := config.Get()

	var apiErr *APIError
	if !errors.As(err, &apiErr) {
		status := http.StatusInternalServerError
		code := "INTERNAL_ERROR"
		msg := "An unexpected error occurred."
		if s.Debug {
			msg = err.Error()
		}
		apiErr = &APIError{Status: status, Code: code, Message: msg}
	}

	WriteJSON(w, apiErr.Status, errorEnvelope{Error: ErrorDetail{
		Code:    apiErr.Code,
		Message: apiErr.Message,
		Fields:  apiErr.FieldErr,
	}})
}

// decodeJSON strictly decodes the request body into dst, producing the same
// VALIDATION_FAILED envelope shape used across the API.
func decodeJSON(r *http.Request, dst interface{}) error {
	defer func() { _, _ = io.Copy(io.Discard, r.Body); r.Body.Close() }()

	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20)) // 1MB body limit
	dec.DisallowUnknownFields()
	if err := dec.Decode(dst); err != nil {
		v := newValidation()
		switch {
		case errors.Is(err, io.EOF):
			v.add("body", "Field required")
		default:
			v.add("body", strings.TrimPrefix(err.Error(), "json: "))
		}
		return v.err()
	}
	return nil
}

// maxBodyBytes guards against oversized payloads on form-less endpoints.
const maxBodyBytes = 1 << 20

// ClientIP extracts the caller IP address from the request (analogous to
// request.client.host, honouring X-Forwarded-For when present).
func ClientIP(r *http.Request) string {
	if xff := r.Header.Get("X-Forwarded-For"); xff != "" {
		parts := strings.Split(xff, ",")
		return strings.TrimSpace(parts[0])
	}
	host := r.RemoteAddr
	if i := strings.LastIndex(host, ":"); i >= 0 {
		host = host[:i]
	}
	if host == "" {
		return "unknown"
	}
	return host
}

// bearerToken returns the credentials from an "Authorization: Bearer x" header.
func bearerToken(r *http.Request) string {
	h := r.Header.Get("Authorization")
	if strings.HasPrefix(h, "Bearer ") {
		return strings.TrimSpace(strings.TrimPrefix(h, "Bearer "))
	}
	return ""
}
