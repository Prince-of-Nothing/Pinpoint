#!/bin/sh
# Container entrypoint: apply migrations, then serve the API.
set -e
alembic upgrade head
exec uvicorn app.main:app --host 0.0.0.0 --port 8000
