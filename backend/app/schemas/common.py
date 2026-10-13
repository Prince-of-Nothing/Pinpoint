"""Base schema configuration shared by all request bodies."""

from pydantic import BaseModel, ConfigDict


class StrictRequest(BaseModel):
    """Unknown fields are rejected (prevents mass assignment); strings are trimmed."""

    model_config = ConfigDict(extra="forbid", str_strip_whitespace=True)


class ApiResponse(BaseModel):
    model_config = ConfigDict(from_attributes=True)
