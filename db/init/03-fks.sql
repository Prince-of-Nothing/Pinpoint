-- Referential integrity for the shared modular database.
-- Domains: user data (users, friendships, shares) <-> location data
-- (locations, check_ins, emergencies) stay connected by FKs while each
-- service owns only its own writes. Idempotent (safe to run repeatedly).
-- Applied by the superuser bootstrap (init scripts + manual runs).
--
-- On a fresh volume the init scripts run before any service has created the
-- tables. In that case this script skips with a notice; re-run it together with
-- 02-grants.sql after the services have booted once.

DO $$
BEGIN
  IF to_regclass('public.pinpoint_users') IS NULL
     OR to_regclass('public.pinpoint_friendships') IS NULL
     OR to_regclass('public.pinpoint_shares') IS NULL
     OR to_regclass('public.pinpoint_locations') IS NULL
     OR to_regclass('public.pinpoint_checkins') IS NULL
     OR to_regclass('public.pinpoint_emergencies') IS NULL THEN
    RAISE NOTICE 'Pinpoint tables are not created yet; re-run 03-fks.sql after the services first boot.';
    RETURN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_friendship_requester') THEN
    ALTER TABLE pinpoint_friendships
      ADD CONSTRAINT fk_friendship_requester FOREIGN KEY (requester_id) REFERENCES pinpoint_users(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_friendship_receiver') THEN
    ALTER TABLE pinpoint_friendships
      ADD CONSTRAINT fk_friendship_receiver FOREIGN KEY (receiver_id) REFERENCES pinpoint_users(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_share_friendship') THEN
    ALTER TABLE pinpoint_shares
      ADD CONSTRAINT fk_share_friendship FOREIGN KEY (friendship_id) REFERENCES pinpoint_friendships(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_share_owner') THEN
    ALTER TABLE pinpoint_shares
      ADD CONSTRAINT fk_share_owner FOREIGN KEY (owner_id) REFERENCES pinpoint_users(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_location_user') THEN
    ALTER TABLE pinpoint_locations
      ADD CONSTRAINT fk_location_user FOREIGN KEY (user_id) REFERENCES pinpoint_users(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_checkin_user') THEN
    ALTER TABLE pinpoint_checkins
      ADD CONSTRAINT fk_checkin_user FOREIGN KEY (user_id) REFERENCES pinpoint_users(id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_emergency_user') THEN
    ALTER TABLE pinpoint_emergencies
      ADD CONSTRAINT fk_emergency_user FOREIGN KEY (user_id) REFERENCES pinpoint_users(id);
  END IF;
END $$;
