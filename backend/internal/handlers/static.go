// Static SPA hosting: serves the bundled web client when the binary is
// started with WEB_ROOT (e.g. WEB_ROOT=./web/public). API routes take
// precedence; unknown non-API paths fall back to index.html (SPA routing).
package handlers

import (
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

func (a *App) staticFileServer() http.Handler {
	root := os.Getenv("WEB_ROOT")
	if root == "" {
		return nil
	}
	abs, err := filepath.Abs(root)
	if err != nil {
		return nil
	}
	indexPath := filepath.Join(abs, "index.html")
	if _, err := os.Stat(indexPath); err != nil {
		return nil
	}
	fs := http.FileServer(http.Dir(abs))
	index, err := os.ReadFile(indexPath)
	if err != nil {
		return nil
	}

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		p := filepath.Clean(filepath.Join(abs, "."+filepath.FromSlash(r.URL.Path)))
		// Prevent path traversal outside the web root.
		if !strings.HasPrefix(p, abs+string(os.PathSeparator)) && p != abs {
			http.NotFound(w, r)
			return
		}
		if st, statErr := os.Stat(p); statErr == nil && !st.IsDir() {
			fs.ServeHTTP(w, r)
			return
		}
		// SPA fallback: serve index.html for client-side routes.
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = w.Write(index)
	})
}
