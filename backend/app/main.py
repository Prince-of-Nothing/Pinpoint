"""
FastAPI application factory and middleware setup.
"""

from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from starlette.middleware.sessions import SessionMiddleware

from app.core.config import get_settings
from app.db.session import init_db
from app.api import auth, users, friendships, locations

settings = get_settings()


def create_application() -> FastAPI:
    """Create and configure the FastAPI application."""
    
    app = FastAPI(
        title=settings.app_name,
        description="Privacy-first location sharing API with directional authorization",
        version="0.1.0",
        debug=settings.debug
    )
    
    # Configure CORS
    app.add_middleware(
        CORSMiddleware,
        allow_origins=settings.allowed_origins,
        allow_credentials=True,
        allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
        allow_headers=["*"],
        expose_headers=["X-CSRF-Token"],
        max_age=600
    )
    
    # Custom exception handler for validation errors
    @app.exception_handler(RequestValidationError)
    async def validation_exception_handler(request: Request, exc: RequestValidationError):
        """Return consistent validation error format."""
        errors = {}
        for error in exc.errors():
            field = ".".join(str(x) for x in error["loc"][1:])  # Skip 'body'
            if field not in errors:
                errors[field] = []
            errors[field].append(error["msg"])
        
        return JSONResponse(
            status_code=422,
            content={
                "error": {
                    "code": "VALIDATION_FAILED",
                    "message": "Validation failed.",
                    "fields": errors
                }
            }
        )
    
    # Generic exception handler to prevent information leakage
    @app.exception_handler(Exception)
    async def generic_exception_handler(request: Request, exc: Exception):
        """Handle unexpected errors without exposing internal details."""
        if settings.debug:
            # Only show details in debug mode
            return JSONResponse(
                status_code=500,
                content={"error": {"code": "INTERNAL_ERROR", "message": str(exc)}}
            )
        
        # Production: generic error message
        return JSONResponse(
            status_code=500,
            content={
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": "An unexpected error occurred."
                }
            }
        )
    
    # Include routers
    app.include_router(auth.router, prefix="/api/v1/auth", tags=["Authentication"])
    app.include_router(users.router, prefix="/api/v1/users", tags=["Users"])
    app.include_router(friendships.router, prefix="/api/v1/friendships", tags=["Friendships"])
    app.include_router(locations.router, prefix="/api/v1/locations", tags=["Locations"])
    
    # Health check endpoint
    @app.get("/health", tags=["Health"])
    async def health_check():
        """Liveness and readiness health check."""
        return {"status": "healthy", "version": "0.1.0"}
    
    # Root endpoint
    @app.get("/")
    async def root():
        """Root endpoint with API information."""
        return {
            "name": settings.app_name,
            "version": "0.1.0",
            "docs": "/docs",
            "health": "/health"
        }
    
    return app


# Create application instance
app = create_application()


@app.on_event("startup")
async def startup_event():
    """Initialize database on startup."""
    if settings.debug:
        print("Initializing database...")
        init_db()
        print("Database initialized.")
