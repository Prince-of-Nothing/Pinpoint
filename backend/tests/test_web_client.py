"""The web client is served same-origin with a strict CSP and no unsafe DOM sinks."""

from pathlib import Path

WEB = Path(__file__).resolve().parent.parent / "web"


def test_index_served_with_csp(client):
    response = client.get("/")
    assert response.status_code == 200
    assert "Always Together" in response.text
    csp = response.headers["content-security-policy"]
    assert "script-src 'self'" in csp and "unsafe-inline" not in csp and "unsafe-eval" not in csp
    assert "frame-ancestors 'none'" in csp


def test_csp_tile_source_is_policy_compliant(client):
    """Tiles must come from an app-friendly host, never OSMF's own servers.

    OSMF's Tile Usage Policy prohibits using tile.openstreetmap.org as an
    app backend; the client (and this CSP) use the Wikimedia Foundation
    endpoint instead. Regression: don't silently revert the tile host.
    """
    csp = client.get("/").headers["content-security-policy"]
    assert "https://maps.wikimedia.org" in csp
    assert "tile.openstreetmap.org" not in csp
    app_js = client.get("/app.js").text
    assert "maps.wikimedia.org" in app_js
    assert "tile.openstreetmap.org" not in app_js


def test_static_assets_served(client):
    for path in ("/app.js", "/styles.css", "/vendor/leaflet/leaflet.js"):
        assert client.get(path).status_code == 200, path


def test_api_404_still_problem_details(client):
    response = client.get("/api/v1/nope")
    assert response.status_code == 404
    assert response.json()["code"] == "NOT_FOUND"


def test_client_code_uses_no_unsafe_html_sinks():
    source = (WEB / "app.js").read_text()
    for sink in ("innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval("):
        assert sink not in source, sink


def test_no_inline_scripts_or_handlers_in_html():
    html = (WEB / "index.html").read_text()
    assert "<script>" not in html
    assert " onclick=" not in html and " onload=" not in html
