-- Per-table least-privilege grants (run AFTER services have created tables:
--   docker exec -i pinpoint-db psql -U pinpoint -d pinpoint < db/02-grants.sql
-- or via the bootstrap target). Owner-mapping mirrors ServiceKind in
-- src/server/state.rs: each binary writes only its own tables. Re-run
-- db/init/03-fks.sql in the same pass so the foreign keys are applied after the
-- tables exist.
--
-- auth service (pinpoint-auth): credentials + MFA fields.
GRANT ALL ON pinpoint_users, pinpoint_state TO pinpoint_auth;
GRANT ALL ON pinpoint_counters TO pinpoint_auth;
--
-- users service (pinpoint-users): profile + friendships (+ own counters).
GRANT ALL ON pinpoint_users, pinpoint_friendships, pinpoint_state TO pinpoint_users;
GRANT ALL ON pinpoint_counters TO pinpoint_users;
-- admin service (pinpoint-admin) reuses the users role and adds observability.
GRANT SELECT, UPDATE ON pinpoint_sessions TO pinpoint_users;
-- Account deletion must remove the user's message key and both directions.
GRANT DELETE ON pinpoint_messages, pinpoint_message_keys TO pinpoint_users;
GRANT SELECT, INSERT, UPDATE ON pinpoint_diagnostic_reports TO pinpoint_users;
GRANT SELECT, INSERT ON pinpoint_admin_audit TO pinpoint_users;
GRANT USAGE, SELECT ON SEQUENCE pinpoint_report_id_seq, pinpoint_audit_id_seq TO pinpoint_users;
--
-- diag service (pinpoint-diag): reports only; no credentials, no audit writes.
GRANT SELECT ON pinpoint_friendships, pinpoint_shares,
      pinpoint_locations, pinpoint_checkins, pinpoint_emergencies,
      pinpoint_diagnostic_reports TO pinpoint_diag;
-- Credential columns (password_hash, totp_secret, recovery_code_hash, identity_subject)
-- stay with the auth and users roles. Revoke the old table-level grant first.
REVOKE SELECT ON pinpoint_users FROM pinpoint_diag, pinpoint_locations, pinpoint_messages;
GRANT SELECT (id, email, full_name, role, is_active, must_change_password, mfa_enabled, mfa_last_step, failed_login_attempts, locked_until, auth_method, public_key) ON pinpoint_users TO pinpoint_diag;
GRANT SELECT, UPDATE ON pinpoint_sessions TO pinpoint_diag;
GRANT INSERT ON pinpoint_diagnostic_reports TO pinpoint_diag;
GRANT UPDATE (next_report_id) ON pinpoint_counters TO pinpoint_diag;
GRANT USAGE, SELECT ON SEQUENCE pinpoint_report_id_seq TO pinpoint_diag;
--
-- locations service (pinpoint-locations): shares/locations/check-ins/emergencies;
-- read-only users+friendships for current_user/consent checks.
GRANT ALL ON pinpoint_shares, pinpoint_locations, pinpoint_checkins, pinpoint_emergencies TO pinpoint_locations;
GRANT SELECT ON pinpoint_friendships TO pinpoint_locations;
GRANT SELECT (id, email, full_name, role, is_active, must_change_password, mfa_enabled, mfa_last_step, failed_login_attempts, locked_until, auth_method, public_key) ON pinpoint_users TO pinpoint_locations;
GRANT UPDATE (next_share_id, next_checkin_id, next_emergency_id) ON pinpoint_counters TO pinpoint_locations;
-- Private place notes: written by locations, removed with the account by users.
GRANT ALL ON pinpoint_place_notes TO pinpoint_locations;
GRANT USAGE, SELECT ON SEQUENCE pinpoint_place_notes_id_seq TO pinpoint_locations;
GRANT DELETE ON pinpoint_place_notes TO pinpoint_users;
--
-- messages service (pinpoint-messages): bundles + opaque envelopes; read-only
-- users/friendships for the accepted-friends check and display names; shared
-- sessions for authentication.
GRANT ALL ON pinpoint_message_keys, pinpoint_messages TO pinpoint_messages;
GRANT SELECT ON pinpoint_friendships TO pinpoint_messages;
GRANT SELECT (id, email, full_name, role, is_active, must_change_password, mfa_enabled, mfa_last_step, failed_login_attempts, locked_until, auth_method, public_key) ON pinpoint_users TO pinpoint_messages;
GRANT SELECT, UPDATE ON pinpoint_sessions TO pinpoint_messages;
GRANT USAGE, SELECT ON SEQUENCE pinpoint_messages_id_seq TO pinpoint_messages;
--
-- Every service applies migrations at boot and records what it applied.
GRANT SELECT, INSERT ON pinpoint_schema_migrations TO pinpoint_auth, pinpoint_users, pinpoint_locations, pinpoint_messages, pinpoint_diag;
--
-- map service: no database access (no DATABASE_URL).
--
-- Deny-by-default: revoke from PUBLIC and don't grant anything else.
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
