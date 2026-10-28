"""
RFC 7807 Problem Details helpers (see docs/reference/api-contract.md).

Every error response uses the same envelope: type/title/status/detail plus a
machine-readable error_code. Internal details never cross the boundary.
"""

from typing import Any

from fastapi import Request
from fastapi.responses import JSONResponse

PROBLEM_TYPES = {
    400: "invalid-request",
    401: "unauthorized",
    403: "forbidden",
    404: "not-found",
    405: "method-not-allowed",
    409: "conflict",
    422: "validation-error",
    429: "rate-limit-exceeded",
    500: "internal-error",
}

DEFAULT_TITLES = {
    400: "Bad Request",
    401: "Unauthorized",
    403: "Forbidden",
    404: "Not Found",
    405: "Method Not Allowed",
    409: "Conflict",
    422: "Validation Failed",
    429: "Too Many Requests",
    500: "Internal Server Error",
}


def problem(
    request: Request,
    status_code: int,
    error_code: str,
    message: str,
    fields: dict[str, Any] | None = None,
    headers: dict[str, str] | None = None,
) -> JSONResponse:
    """Build an RFC 7807 response."""
    payload: dict[str, Any] = {
        "type": f"about://errors/{PROBLEM_TYPES.get(status_code, 'error')}",
        "title": DEFAULT_TITLES.get(status_code, "Error"),
        "status": status_code,
        "detail": message,
        "instance": request.url.path if request else None,
        "error_code": error_code,
    }
    if fields:
        payload["fields"] = fields
    return JSONResponse(status_code=status_code, content=payload, headers=headers)
