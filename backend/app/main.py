"""FastAPI application factory."""

import logging

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from app import models  # noqa: F401  (registers tables)
from app.api import auth, blocks, friendships, location_shares, locations, users
from app.core.config import get_settings
from app.core.errors import install_error_handlers
from app.core.middleware import BodySizeLimitMiddleware, SecurityHeadersMiddleware

API_PREFIX = "/api/v1"


def create_app() -> FastAPI:
    settings = get_settings()
    logging.basicConfig(level=logging.DEBUG if settings.debug else logging.INFO)

    is_production = settings.environment == "production"
    app = FastAPI(
        title=settings.app_name,
        description="Privacy-first location sharing API with directional consent.",
        version="0.2.0",
        debug=settings.debug,
        # Interactive docs are useful in development but reveal the attack surface in production
        docs_url=None if is_production else "/docs",
        redoc_url=None,
        openapi_url=None if is_production else "/openapi.json",
    )

    install_error_handlers(app)

    if settings.allowed_origins:
        app.add_middleware(
            CORSMiddleware,
            allow_origins=settings.allowed_origins,
            allow_credentials=True,
            allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE"],
            allow_headers=["Authorization", "Content-Type", settings.csrf_header_name],
            max_age=600,
        )
    app.add_middleware(BodySizeLimitMiddleware, max_bytes=settings.max_request_body_bytes)
    app.add_middleware(SecurityHeadersMiddleware, hsts=is_production)

    app.include_router(auth.router, prefix=f"{API_PREFIX}/auth", tags=["Authentication"])
    app.include_router(users.router, prefix=f"{API_PREFIX}/users", tags=["Users"])
    app.include_router(friendships.router, prefix=f"{API_PREFIX}/friendships", tags=["Friendships"])
    app.include_router(blocks.router, prefix=f"{API_PREFIX}/blocks", tags=["Blocks"])
    app.include_router(
        location_shares.router, prefix=f"{API_PREFIX}/location-shares", tags=["Location sharing"]
    )
    app.include_router(locations.router, prefix=f"{API_PREFIX}/locations", tags=["Locations"])

    @app.get(f"{API_PREFIX}/health", tags=["Health"])
    def health() -> dict:
        return {"status": "ok", "version": app.version}

    return app


app = create_app()
