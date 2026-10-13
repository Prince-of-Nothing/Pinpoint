// Tile tests: path safety, bbox math, manifest discovery.
package tiles_test

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/always-together/backend/internal/tiles"
)

func TestSafeJoinRejectsTraversal(t *testing.T) {
	for _, bad := range []string{
		"../etc/passwd", "12/../../x.png", "12/../x/y.png", "20/1/1.png",
		"12/4096/1360.png", "12/1/1.jpg", "abc", "",
	} {
		if p := tiles.SafeJoin("/tmp/root", bad); p != "" {
			t.Errorf("SafeJoin(%q) = %q, want rejected", bad, p)
		}
	}
	if p := tiles.SafeJoin("/tmp/root", "12/2048/1360.png"); p == "" {
		t.Error("valid tile path was rejected")
	}
}

func TestRangeForBBoxParis(t *testing.T) {
	// Central Paris at z12 should be a small handful of tiles around 2048/1360.
	tr, err := tiles.RangeForBBox(12, 48.81, 2.29, 48.90, 2.40)
	if err != nil {
		t.Fatal(err)
	}
	if tr.Z != 12 || tr.TotalTiles < 1 {
		t.Fatalf("unexpected range %+v", tr)
	}
	found := false
	for _, rel := range tr.Tiles() {
		if rel == "12/2074/1409.png" {
			found = true
		}
	}
	if !found {
		t.Error("expected tile 12/2074/1409.png inside central-Paris range")
	}
}

func TestRangeForBBoxValidation(t *testing.T) {
	if _, err := tiles.RangeForBBox(20, 0, 0, 1, 1); err == nil {
		t.Error("zoom 20 should be rejected")
	}
	if _, err := tiles.RangeForBBox(5, 10, 0, -10, 1); err == nil {
		t.Error("inverted bbox should be rejected")
	}
}

func TestLoadSaveRoundTrip(t *testing.T) {
	root := t.TempDir()
	payload := []byte("\x89PNG fake tile bytes")
	if err := tiles.Save(root, "12/2048/1360.png", payload); err != nil {
		t.Fatal(err)
	}
	if !tiles.Exists(root, "12/2048/1360.png") {
		t.Fatal("tile should exist after save")
	}
	got, err := tiles.Load(root, "12/2048/1360.png")
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(payload) {
		t.Fatalf("roundtrip mismatch: %q", got)
	}
	if _, err := tiles.Load(root, "../escape.png"); err == nil {
		t.Fatal("traversal load must fail")
	}
}

func TestManifestDiscoversPacks(t *testing.T) {
	root := t.TempDir()
	pack := filepath.Join(root, "paris")
	if err := os.MkdirAll(pack, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(pack, "manifest.json"), []byte("{}"), 0o644); err != nil {
		t.Fatal(err)
	}
	m := tiles.Manifest(root)
	packs, ok := m["packs"].([]map[string]any)
	if !ok || len(packs) != 1 || packs[0]["name"] != "paris" {
		t.Fatalf("manifest did not discover pack: %v", m)
	}
}
