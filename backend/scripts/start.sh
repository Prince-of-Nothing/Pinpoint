#!/bin/sh
# Container entrypoint: apply migrations (as the schema owner), then serve the API.
set -e
alembic upgrade head
# Only trust X-Forwarded-* headers from the reverse proxy's address
exec uvicorn app.main:app --host 0.0.0.0 --port 8000 \
    --proxy-headers --forwarded-allow-ips "${FORWARDED_ALLOW_IPS:-127.0.0.1}"
