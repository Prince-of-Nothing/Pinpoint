#!/bin/sh
# Runs once, when the PostgreSQL volume is first initialised.
#
# POSTGRES_USER owns the schema and is used only for migrations.
# app_runtime is what the API uses: it can read and write rows but cannot
# create, alter or drop tables (least privilege, SC-06).
set -eu

psql -v ON_ERROR_STOP=1 -v app_password="$APP_DB_PASSWORD" \
     --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<'EOSQL'
CREATE ROLE app_runtime LOGIN PASSWORD :'app_password';

REVOKE ALL ON DATABASE always_together FROM PUBLIC;
GRANT CONNECT ON DATABASE always_together TO app_runtime;

REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE ON SCHEMA public TO app_runtime;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO app_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO app_runtime;
EOSQL
