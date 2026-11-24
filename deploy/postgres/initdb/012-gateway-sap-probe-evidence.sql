-- Additive extension of 007. Existing rows and interrupted running probes remain unknown.
-- Only a closed, secret-free SAP probe observation writes this nullable snapshot.
ALTER TABLE gateway_internal_model_probe_audits
    ADD COLUMN IF NOT EXISTS sap_evidence JSONB
    CHECK (sap_evidence IS NULL OR jsonb_typeof(sap_evidence) = 'object');
