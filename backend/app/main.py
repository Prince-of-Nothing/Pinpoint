"""
FastAPI application factory and middleware setup.

Design patterns applied:
- Factory: create_application() builds a fully configured app (testable,
  multiple instances possible).
- Decorator: middleware wraps every request with host checks, CORS, and
  security headers (cross-cutting concerns kept out of routers).
- Fail-safe defaults: production boots with docs disabled, generic errors,
  strict host allowlisting, and no stack traces anywhere.
"""

import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request, status
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from fastapi.responses import JSONResponse
from starlette.exceptions import HTTPException as StarletteHTTPException

from app.api import auth, friendships, locations, users
from app.api.errors import problem
from app.core.config import get_settings
from app.db.session import init_db
from app.middleware import SecurityHeadersMiddleware

logger = logging.getLogger("always_together")

settings = get_settings()


def create_application() -> FastAPI:
    """Create and configure the FastAPI application."""

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        """Initialize database on startup (modern replacement for on_event)."""
        if settings.is_production:
            # Production should use Alembic migrations; create_all is a dev/test
            # convenience. Tracked in docs/architecture/risks-and-technical-debt.md.
            logger.warning("Running with auto table creation; wire Alembic for production.")
        init_db()
        logger.info("Database initialized (%s).", settings.environment)
        yield

    app = FastAPI(
        title=settings.app_name,
        description="Privacy-first location sharing API with directional authorization",
        version="0.1.0",
        debug=False,  # never propagate internals via the framework itself
        docs_url="/docs" if settings.docs_enabled else None,
        redoc_url="/redoc" if settings.docs_enabled else None,
        openapi_url="/openapi.json" if settings.docs_enabled else None,
        lifespan=lifespan,
    )

    # --- Exception handlers (RFC 7807, no information leakage) -------------

    @app.exception_handler(StarletteHTTPException)
    async def http_exception_handler(request: Request, exc: StarletteHTTPException):
        """Map raised HTTPExceptions to the RFC 7807 envelope."""
        detail = exc.detail if isinstance(exc.detail, dict) else {"message": str(exc.detail)}
        error_code = detail.pop("code", "ERROR") if isinstance(detail, dict) else "ERROR"
        fields = detail.pop("fields", None) if isinstance(detail, dict) else None
        message = (detail or {}).get("message", str(exc.detail))
        return problem(
            request,
            status_code=exc.status_code,
            error_code=error_code,
            message=message,
            fields=fields,
            headers=getattr(exc, "headers", None),
        )

    @app.exception_handler(RequestValidationError)
    async def validation_exception_handler(request: Request, exc: RequestValidationError):
        """Return a consistent, bounded validation error format."""
        fields = {}
        for error in exc.errors():
            field = ".".join(str(part) for part in error["loc"][1:]) or "body"
            fields.setdefault(field, []).append(error["msg"])
        return problem(
            request,
            status_code=422,  # Unprocessable Content
            error_code="VALIDATION_FAILED",
            message="Validation failed.",
            fields=fields,
        )

    @app.exception_handler(Exception)
    async def generic_exception_handler(request: Request, exc: Exception):
        """Log internally, return a generic error (never a stack trace)."""
        logger.exception("Unhandled error on %s %s", request.method, request.url.path)
        return problem(
            request,
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            error_code="INTERNAL_ERROR",
            message="An unexpected error occurred.",
        )

    # --- Middleware (last added = outermost) -------------------------------

    app.add_middleware(SecurityHeadersMiddleware)
    app.add_middleware(
        TrustedHostMiddleware,
        allowed_hosts=settings.trusted_hosts,  # Host-header poisoning guard
    )
    app.add_middleware(
        CORSMiddleware,
        allow_origins=settings.allowed_origins,
        allow_credentials=True,
        allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
        allow_headers=["Authorization", "Content-Type", "X-CSRF-Token"],
        expose_headers=[
            "X-CSRF-Token",
            "Retry-After",
            "X-RateLimit-Limit",
            "X-RateLimit-Remaining",
            "X-RateLimit-Reset",
        ],
        max_age=600,
    )

    # --- Routers ------------------------------------------------------------

    app.include_router(auth.router, prefix="/api/v1/auth", tags=["Authentication"])
    app.include_router(users.router, prefix="/api/v1/users", tags=["Users"])
    app.include_router(friendships.router, prefix="/api/v1/friendships", tags=["Friendships"])
    app.include_router(locations.router, prefix="/api/v1/locations", tags=["Locations"])

    # --- Health / info -------------------------------------------------------

    @app.get("/health", tags=["Health"])
    async def health_check():
        """Liveness and readiness health check (DB connectivity verified)."""
        from sqlalchemy import text
        from app.db.session import SessionLocal

        database = "unavailable"
        try:
            with SessionLocal() as session:
                session.execute(text("SELECT 1"))
            database = "connected"
        except Exception:
            logger.warning("Health check: database not reachable")

        status_text = "healthy" if database == "connected" else "degraded"
        return {"status": status_text, "database": database, "version": "0.1.0"}

    @app.get("/", tags=["Health"], include_in_schema=False)
    async def root():
        """Root endpoint with API information."""
        return {
            "name": settings.app_name,
            "version": "0.1.0",
            "docs": "/docs" if settings.docs_enabled else None,
            "health": "/health",
        }

    return app


app = create_application()
