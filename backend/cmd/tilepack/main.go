// Command tilepack manages offline OpenStreetMap tile packs.
//
// Usage:
//
//	tilepack --seed [repoDir] [cacheDir]     copy committed maps-requests/*
//	                                        packs into the runtime cache
//	tilepack --fetch <z/x/y.png> [...]       download specific tiles once and
//	                                        store them in the cache
//
// It is run automatically at container start (see backend/Dockerfile) so a
// fresh deployment already contains every map region committed to GitHub —
// clients then download tiles from *our* server, never from OSM directly.
package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/always-together/backend/internal/tiles"
)

func main() {
	args := os.Args[1:]
	if len(args) == 0 {
		usage()
		os.Exit(2)
	}
	cache := envOr("TILES_ROOT", "tiles_cache")

	switch args[0] {
	case "--seed":
		repo := argOr(args, 1, "maps-requests")
		n, err := seedPacks(repo, cache)
		if err != nil {
			fmt.Fprintln(os.Stderr, "tilepack:", err)
			os.Exit(1)
		}
		fmt.Printf("tilepack: seeded %d tile(s) from %s into %s\n", n, repo, cache)
	case "--fetch":
		upstream := envOr("TILES_UPSTREAM", tiles.DefaultUpstream)
		for _, rel := range args[1:] {
			rel = strings.TrimPrefix(rel, "/")
			if tiles.Exists(cache, rel) {
				fmt.Println("cached:", rel)
				continue
			}
			data, err := tiles.Fetch(upstream, rel)
			if err != nil {
				fmt.Fprintln(os.Stderr, "tilepack:", err)
				os.Exit(1)
			}
			if err := tiles.Save(cache, rel, data); err != nil {
				fmt.Fprintln(os.Stderr, "tilepack:", err)
				os.Exit(1)
			}
			fmt.Println("fetched:", rel)
		}
	default:
		usage()
		os.Exit(2)
	}
}

// seedPacks copies every pack directory containing a manifest.json from
// repoDir into cacheDir/<name>/ (idempotent: existing files are skipped).
func seedPacks(repoDir, cacheDir string) (int, error) {
	dirs, err := os.ReadDir(repoDir)
	if err != nil {
		return 0, fmt.Errorf("read %s: %w", repoDir, err)
	}
	copied := 0
	for _, d := range dirs {
		if !d.IsDir() {
			continue
		}
		src := filepath.Join(repoDir, d.Name())
		if _, err := os.Stat(filepath.Join(src, "manifest.json")); err != nil {
			continue // not a tile pack
		}
		dst := filepath.Join(cacheDir, d.Name())
		err := filepath.Walk(src, func(p string, info os.FileInfo, err error) error {
			if err != nil || info.IsDir() {
				return err
			}
			relToSrc, _ := filepath.Rel(src, p)
			target := filepath.Join(dst, relToSrc)
			if _, err := os.Stat(target); err == nil {
				return nil // already seeded
			}
			if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
				return err
			}
			in, err := os.Open(p)
			if err != nil {
				return err
			}
			defer in.Close()
			out, err := os.Create(target)
			if err != nil {
				return err
			}
			defer out.Close()
			if _, err := io.Copy(out, in); err != nil {
				return err
			}
			if strings.HasSuffix(p, ".png") {
				copied++ // count installed tiles
			}
			return nil
		})
		if err != nil {
			return copied, err
		}
	}
	return copied, nil
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func argOr(args []string, i int, def string) string {
	if len(args) > i {
		return args[i]
	}
	return def
}

func usage() {
	fmt.Fprintln(os.Stderr, `usage:
  tilepack --seed [repoDir=maps-requests]   install committed tile packs into $TILES_ROOT
  tilepack --fetch z/x/y.png ...            fetch specific tiles into $TILES_ROOT`)
}
