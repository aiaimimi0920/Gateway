--
-- Standalone-only constraint relaxation.
--
-- `gateway_request_audits.provider_account_id` references
-- `gateway_provider_accounts` because in a Platform-managed deployment every
-- provider account is a database row and Platform is the writer.
--
-- A standalone Gateway keeps its providers and accounts in the route document
-- (`GATEWAY_ROUTES_FILE`) instead, and `gateway_provider_accounts` stays empty.
-- The pipeline still records which provider served each request, so with the
-- foreign key in place every audit finalise fails, rows stay `status='running'`
-- for ever, `/v1/internal/gateway/pressure` reports concurrency that was
-- released long ago, and the console never gets a success rate.
--
-- Dropping the constraint keeps the attribution (the alternative — writing NULL
-- — would destroy exactly the per-provider grouping the console cards need).
-- The only thing lost is the `ON DELETE SET NULL` cleanup for deleted provider
-- accounts, which a standalone deployment never has rows for anyway.
--
-- This file is applied only by the bundled compose stack. A deployment sharing
-- Platform's database keeps the foreign key, and the ids it writes are real
-- `gateway_provider_accounts` rows, so nothing here is needed there.
--

ALTER TABLE public.gateway_request_audits
  DROP CONSTRAINT IF EXISTS gateway_request_audits_provider_account_id_fkey;
