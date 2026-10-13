// Map-tile endpoints: serve OpenStreetMap XYZ tiles from the local pack /
// cache (offline-first), expose available downloadable tile packs, and let
// authenticated users request new map regions ("maps-requests" workflow —
// requests are recorded so maintainers can fetch them into the repo instead
// of every client hammering the public OSM tile servers).
package handlers

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/go-chi/chi/v5"

	"github.com/always-together/backend/internal/tiles"
)

// tileCacheRoot resolves the directory holding cached/served tiles.
func tileCacheRoot() string {
	if r := os.Getenv("TILES_ROOT"); r != "" {
		return r
	}
	return "tiles_cache"
}

func tileUpstream() string {
	if u := os.Getenv("TILES_UPSTREAM"); u != "" {
		return u
	}
	return tiles.DefaultUpstream
}

// GET /tiles/{z}/{x}/{y}.png  (public; rate-limit friendly, long cache TTL)
//
// Serves the tile from the local cache when present. On a miss it back-fills
// exactly once from the upstream mirror and caches it locally, so repeat
// visitors never add load to the public tile servers.
func (a *App) handleTile(w http.ResponseWriter, r *http.Request) {
	z := chi.URLParam(r, "z")
	x := chi.URLParam(r, "x")
	y := strings.TrimSuffix(chi.URLParam(r, "y"), ".png")
	rel := z + "/" + x + "/" + y + ".png"

	root := tileCacheRoot()
	data, err := tiles.Get(root, tileUpstream(), rel)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "TILE_NOT_AVAILABLE",
			"Tile not in any local pack and could not be fetched. Request this area via POST /api/v1/maps/requests."))
		return
	}
	w.Header().Set("Content-Type", "image/png")
	w.Header().Set("Cache-Control", "public, max-age=604800, immutable")
	w.Header().Set("X-OSM-Data", "(c) OpenStreetMap contributors, ODbL — https://www.openstreetmap.org/copyright")
	_, _ = w.Write(data)
}

// GET /api/v1/maps/packs  (public)
// Lists downloadable tile packs available on this deployment.
func (a *App) handleMapPacks(w http.ResponseWriter, r *http.Request) {
	WriteJSON(w, http.StatusOK, tiles.Manifest(tileCacheRoot()))
}

// GET /api/v1/maps/packs/{name}/manifest  (public)
// Returns a pack's manifest.json (bbox, zooms, tile list with sizes+hashes)
// so clients can plan selective downloads.
func (a *App) handleMapPackManifest(w http.ResponseWriter, r *http.Request) {
	name := sanitizePackName(chi.URLParam(r, "name"))
	p := filepath.Join(tileCacheRoot(), name, "manifest.json")
	if name == "" || !strings.HasPrefix(p, filepath.Clean(tileCacheRoot())) {
		WriteError(w, r, errf(http.StatusBadRequest, "INVALID_PACK", "Invalid pack name."))
		return
	}
	data, err := os.ReadFile(p)
	if err != nil {
		WriteError(w, r, errf(http.StatusNotFound, "PACK_NOT_FOUND", "Tile pack not found."))
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_, _ = w.Write(data)
}

type mapRequest struct {
	Name      string    `json:"name"`
	South     float64   `json:"south"`
	West      float64   `json:"west"`
	North     float64   `json:"north"`
	East      float64   `json:"east"`
	MinZoom   int       `json:"min_zoom"`
	MaxZoom   int       `json:"max_zoom"`
	Requested time.Time `json:"requested_at"`
	ByUserID  int64     `json:"-"`
}

var (
	mapReqMu        sync.Mutex
	pendingRequests []mapRequest
)

const maxPendingRequests = 200

// POST /api/v1/maps/requests  (authenticated)
// Records a request for an offline map region. Requests are queued here and
// exported by maintainers to scripts/fetch_tiles.py, which downloads the
// tiles ONCE and commits them under maps-requests/<name>/ — clients then
// download from our GitHub instead of the OSM servers.
func (a *App) handleMapRegionRequest(w http.ResponseWriter, r *http.Request) {
	user := userFromContext(r.Context())
	var body struct {
		Name    string  `json:"name"`
		South   float64 `json:"south"`
		West    float64 `json:"west"`
		North   float64 `json:"north"`
		East    float64 `json:"east"`
		MinZoom int     `json:"min_zoom"`
		MaxZoom int     `json:"max_zoom"`
	}
	if err := decodeJSON(r, &body); err != nil {
		WriteError(w, r, err)
		return
	}
	name := sanitizePackName(body.Name)
	if name == "" {
		WriteError(w, r, errf(http.StatusBadRequest, "INVALID_NAME",
			"Provide a region name (letters, digits, dash, underscore)."))
		return
	}
	if body.South >= body.North || body.West >= body.East ||
		body.South < -90 || body.North > 90 || body.West < -180 || body.East > 180 {
		WriteError(w, r, errf(http.StatusBadRequest, "INVALID_BBOX", "Invalid bounding box."))
		return
	}
	if body.MinZoom < 0 || body.MaxZoom > 14 || body.MinZoom > body.MaxZoom {
		WriteError(w, r, errf(http.StatusBadRequest, "INVALID_ZOOM",
			"Zoom range must satisfy 0 <= min_zoom <= max_zoom <= 14."))
		return
	}
	total := 0
	for z := body.MinZoom; z <= body.MaxZoom; z++ {
		tr, err := tiles.RangeForBBox(z, body.South, body.West, body.North, body.East)
		if err != nil {
			WriteError(w, r, errf(http.StatusBadRequest, "INVALID_BBOX", err.Error()))
			return
		}
		total += tr.TotalTiles
	}
	if total > 50000 {
		WriteError(w, r, errf(http.StatusBadRequest, "REGION_TOO_LARGE",
			"Region would require more than 50000 tiles; narrow the bbox or zoom range."))
		return
	}

	reqItem := mapRequest{
		Name: name, South: body.South, West: body.West, North: body.North, East: body.East,
		MinZoom: body.MinZoom, MaxZoom: body.MaxZoom,
		Requested: time.Now().UTC(), ByUserID: user.ID,
	}
	mapReqMu.Lock()
	if len(pendingRequests) < maxPendingRequests {
		pendingRequests = append(pendingRequests, reqItem)
	}
	mapReqMu.Unlock()

	WriteJSON(w, http.StatusAccepted, map[string]any{
		"status":      "queued",
		"name":        name,
		"total_tiles": total,
		"fetch_script": "python3 scripts/fetch_tiles.py --name " + name +
			fmt.Sprintf(" --bbox %.6f,%.6f,%.6f,%.6f --zoom %d-%d", body.South, body.West, body.North, body.East, body.MinZoom, body.MaxZoom),
		"note": "Maintainers fetch requested regions into the repo (maps-requests/) so all clients download from GitHub, not OSM servers.",
	})
}

// GET /api/v1/maps/requests  (admin)
// Lets admins/moderators review the pending queue of map-region requests.
func (a *App) handleListMapRequests(w http.ResponseWriter, r *http.Request) {
	mapReqMu.Lock()
	out := make([]mapRequest, len(pendingRequests))
	copy(out, pendingRequests)
	mapReqMu.Unlock()
	WriteJSON(w, http.StatusOK, map[string]any{"requests": out})
}

func sanitizePackName(s string) string {
	s = strings.TrimSpace(strings.ToLower(s))
	if s == "" || len(s) > 64 {
		return ""
	}
	var b strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' || r == '_' {
			b.WriteRune(r)
		} else {
			return "" // reject anything else (path separators, dots, unicode…)
		}
	}
	return b.String()
}
