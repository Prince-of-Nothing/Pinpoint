"""
Security-headers middleware.

Design pattern: Decorator — every response passes through a fixed set of
hardening headers (defence in depth at the transport boundary):

- HSTS: forces HTTPS after first contact (production / TLS requests).
- nosniff / frame DENY / no-referrer: classic browser-side protections for a
  JSON API that may be opened via a web companion.
- CSP `default-src 'none'`: an API response must never be interpreted as an
  active document (mitigations for MIME sniffing and any future HTML leak).
- COOP/COEP: isolates the browsing context.
"""

from starlette.middleware.base import BaseHTTPMiddleware, RequestResponseEndpoint
from starlette.requests import Request
from starlette.responses import Response

from app.core.config import get_settings

settings = get_settings()

# Swagger UI needs its CDN assets; only these paths skip the strict CSP.
DOCS_PATHS = ("/docs", "/redoc", "/openapi.json")

STATIC_SECURITY_HEADERS = {
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Referrer-Policy": "no-referrer",
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Resource-Policy": "same-origin",
    "Permissions-Policy": "geolocation=(), microphone=(), camera=()",
}

API_CSP = "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"


class SecurityHeadersMiddleware(BaseHTTPMiddleware):
    """Attach hardening headers to every response."""

    async def dispatch(self, request: Request, call_next: RequestResponseEndpoint) -> Response:
        response = await call_next(request)

        for header, value in STATIC_SECURITY_HEADERS.items():
            response.headers.setdefault(header, value)

        if request.url.path not in DOCS_PATHS:
            response.headers.setdefault("Content-Security-Policy", API_CSP)

        # HSTS only makes sense over TLS (or in production behind TLS).
        if request.url.scheme == "https" or settings.is_production:
            response.headers.setdefault(
                "Strict-Transport-Security", "max-age=31536000; includeSubDomains"
            )

        # Never advertise the framework/version stack.
        for header in ("Server", "X-Powered-By"):
            if header in response.headers:
                del response.headers[header]

        return response
