-- Pinpoint — target relational schema (design reference)
--
-- This file is the **design target** described in the wiki Database-Schema page.
-- It is additive and is not applied by the runtime yet; the migration plan
-- lives in that document. Sensitive columns are application-encrypted
-- (`*_enc`, format `v1:<nonce>:`), lookups use blind indexes (`*_hash`).

BEGIN;

-- ---------------------------------------------------------------------------
-- Identity
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id                    BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    email_enc             TEXT        NOT NULL,
    email_hash            TEXT        NOT NULL UNIQUE,
    full_name_enc         TEXT        NOT NULL,
    role                  TEXT        NOT NULL DEFAULT 'resident',
    is_active             BOOLEAN     NOT NULL DEFAULT true,
    must_change_password  BOOLEAN     NOT NULL DEFAULT false,
    auth_method           TEXT        NOT NULL DEFAULT 'email'
                          CHECK (auth_method IN ('email','anonymous','oauth','passkey')),
    public_key            TEXT,
    password_hash          TEXT,
    recovery_code_hash    TEXT,
    failed_login_attempts INT         NOT NULL DEFAULT 0,
    locked_until          TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS users_auth_method_ix ON users (auth_method);

CREATE TABLE IF NOT EXISTS sessions (
    session_id     TEXT        PRIMARY KEY,
    user_id        BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf_token_hash TEXT       NOT NULL,
    auth_method    TEXT        NOT NULL,
    mfa_pending    BOOLEAN     NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_activity  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS sessions_user_ix ON sessions (user_id);
CREATE INDEX IF NOT EXISTS sessions_activity_ix ON sessions (last_activity);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    token_hash   TEXT        PRIMARY KEY,
    session_id   TEXT        NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    expires_at   TIMESTAMPTZ NOT NULL,
    rotated_from TEXT,
    used_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS refresh_tokens_session_ix ON refresh_tokens (session_id);

CREATE TABLE IF NOT EXISTS mfa_secrets (
    user_id   BIGINT      PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret_enc TEXT       NOT NULL,
    enabled   BOOLEAN     NOT NULL DEFAULT false,
    last_step BIGINT      NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS oauth_identities (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id      BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider     TEXT        NOT NULL,
    subject_hash TEXT        NOT NULL UNIQUE,
    subject_enc  TEXT        NOT NULL,
    email_enc    TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS oauth_user_ix ON oauth_identities (user_id);

CREATE TABLE IF NOT EXISTS passkeys (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id TEXT        NOT NULL UNIQUE,
    public_key    TEXT        NOT NULL,
    sign_count    BIGINT      NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS passkeys_user_ix ON passkeys (user_id);

CREATE TABLE IF NOT EXISTS recovery_codes (
    id        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id   BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT        NOT NULL,
    used_at   TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS recovery_user_ix ON recovery_codes (user_id);

CREATE TABLE IF NOT EXISTS user_settings (
    user_id  BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings JSONB  NOT NULL DEFAULT '{}'::jsonb,
    version  INT    NOT NULL DEFAULT 1
);

-- ---------------------------------------------------------------------------
-- Social
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS friendships (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    requester_id BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    receiver_id  BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status       TEXT        NOT NULL
                 CHECK (status IN ('pending','accepted','rejected','blocked')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    responded_at TIMESTAMPTZ,
    UNIQUE (requester_id, receiver_id),
    CHECK (requester_id <> receiver_id)
);
CREATE INDEX IF NOT EXISTS friendships_receiver_status_ix
    ON friendships (receiver_id, status);

CREATE TABLE IF NOT EXISTS blocks (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    blocker_id BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_id BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);

-- ---------------------------------------------------------------------------
-- Location
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS shares (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    friendship_id   BIGINT      NOT NULL REFERENCES friendships(id) ON DELETE CASCADE,
    owner_id        BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    direction       TEXT        NOT NULL
                    CHECK (direction IN ('to_friend','from_friend','mutual')),
    precision_level TEXT        NOT NULL
                    CHECK (precision_level IN ('exact','approximate','city_only')),
    is_active       BOOLEAN     NOT NULL DEFAULT true,
    label_enc       TEXT,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS shares_friendship_ix ON shares (friendship_id);
CREATE INDEX IF NOT EXISTS shares_owner_ix ON shares (owner_id);
CREATE INDEX IF NOT EXISTS shares_expires_ix ON shares (expires_at)
    WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS locations (
    user_id         BIGINT      PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    lat_enc         TEXT        NOT NULL,
    lon_enc         TEXT        NOT NULL,
    accuracy_enc    TEXT,
    precision_level TEXT        NOT NULL
                    CHECK (precision_level IN ('exact','approximate','city_only')),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS check_ins (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    lat_enc    TEXT        NOT NULL,
    lon_enc    TEXT        NOT NULL,
    note_enc   TEXT,
    kind       TEXT        NOT NULL DEFAULT 'manual',
    status     TEXT        NOT NULL DEFAULT 'open'
               CHECK (status IN ('open','completed','cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS checkins_user_time_ix
    ON check_ins (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS emergencies (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    lat_enc     TEXT        NOT NULL,
    lon_enc     TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS emergencies_user_time_ix
    ON emergencies (user_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Audit (append-only)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS audit_events (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_id    BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT        NOT NULL,
    target_type TEXT,
    target_id   BIGINT,
    outcome     TEXT        NOT NULL,
    metadata_enc TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS audit_actor_time_ix
    ON audit_events (actor_id, created_at DESC);

COMMIT;
