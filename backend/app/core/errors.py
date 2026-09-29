"""
Consistent, non-leaking error responses in RFC 9457 (Problem Details) format.

Every error carries a stable machine-readable ``code``; internal details
(stack traces, SQL, exception text) are never returned to clients.
"""

import logging
from http import HTTPStatus

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from starlette.exceptions import HTTPException as StarletteHTTPException

logger = logging.getLogger("app.errors")

PROBLEM_CONTENT_TYPE = "application/problem+json"


class ProblemError(Exception):
    """Raise to return a Problem Details response."""

    def __init__(
        self,
        status: int,
        code: str,
        detail: str,
        *,
        errors: dict[str, list[str]] | None = None,
        headers: dict[str, str] | None = None,
    ) -> None:
        super().__init__(detail)
        self.status = status
        self.code = code
        self.detail = detail
        self.errors = errors
        self.headers = headers


def problem_response(
    request: Request,
    status: int,
    code: str,
    detail: str,
    errors: dict[str, list[str]] | None = None,
    headers: dict[str, str] | None = None,
) -> JSONResponse:
    body: dict = {
        "type": f"about:blank#{code.lower()}",
        "title": HTTPStatus(status).phrase,
        "status": status,
        "code": code,
        "detail": detail,
        "instance": request.url.path,
    }
    if errors:
        body["errors"] = errors
    return JSONResponse(body, status_code=status, headers=headers, media_type=PROBLEM_CONTENT_TYPE)


# Shorthand constructors for the most common cases
def not_found(what: str = "Resource") -> ProblemError:
    return ProblemError(404, "NOT_FOUND", f"{what} not found.")


def forbidden(detail: str = "You are not allowed to perform this action.") -> ProblemError:
    return ProblemError(403, "FORBIDDEN", detail)


def conflict(detail: str) -> ProblemError:
    return ProblemError(409, "CONFLICT", detail)


def bad_request(code: str, detail: str) -> ProblemError:
    return ProblemError(400, code, detail)


_STATUS_CODES = {
    400: "BAD_REQUEST",
    401: "UNAUTHORIZED",
    403: "FORBIDDEN",
    404: "NOT_FOUND",
    405: "METHOD_NOT_ALLOWED",
    409: "CONFLICT",
    413: "PAYLOAD_TOO_LARGE",
    415: "UNSUPPORTED_MEDIA_TYPE",
    429: "RATE_LIMITED",
}


def install_error_handlers(app: FastAPI) -> None:
    @app.exception_handler(ProblemError)
    async def _problem(request: Request, exc: ProblemError):
        return problem_response(request, exc.status, exc.code, exc.detail, exc.errors, exc.headers)

    @app.exception_handler(RequestValidationError)
    async def _validation(request: Request, exc: RequestValidationError):
        errors: dict[str, list[str]] = {}
        for err in exc.errors():
            loc = [
                str(part) for part in err.get("loc", ()) if part not in ("body", "query", "path")
            ]
            field = ".".join(loc) or "request"
            # Never echo the rejected input back (it may contain secrets or payloads)
            errors.setdefault(field, []).append(err.get("msg", "Invalid value"))
        return problem_response(
            request, 422, "VALIDATION_FAILED", "The request is invalid.", errors
        )

    @app.exception_handler(StarletteHTTPException)
    async def _http(request: Request, exc: StarletteHTTPException):
        code = _STATUS_CODES.get(exc.status_code, "HTTP_ERROR")
        detail = exc.detail if isinstance(exc.detail, str) else HTTPStatus(exc.status_code).phrase
        return problem_response(
            request, exc.status_code, code, detail, headers=getattr(exc, "headers", None)
        )

    @app.exception_handler(Exception)
    async def _unhandled(request: Request, exc: Exception):
        logger.exception("Unhandled error on %s %s", request.method, request.url.path)
        return problem_response(request, 500, "INTERNAL_ERROR", "An unexpected error occurred.")
