--
-- PostgreSQL database dump
--


-- Dumped from database version 16.14 (Debian 16.14-1.pgdg12+1)
-- Dumped by pg_dump version 16.14 (Debian 16.14-1.pgdg12+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: gateway_access_bundle_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_bundle_items (
    bundle_id text NOT NULL,
    platform_access_id text NOT NULL,
    created_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_access_bundles; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_bundles (
    id text NOT NULL,
    project_id text,
    slug text NOT NULL,
    display_name text NOT NULL,
    status text NOT NULL,
    description text,
    metadata jsonb,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    billing_mode text DEFAULT 'time_pass'::text NOT NULL
);


--
-- Name: gateway_access_key_aggregate_memberships; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_key_aggregate_memberships (
    aggregate_access_key_id text NOT NULL,
    member_access_key_id text NOT NULL,
    priority integer DEFAULT 100 NOT NULL,
    created_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_access_key_balances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_key_balances (
    access_key_id text NOT NULL,
    balance_mode text NOT NULL,
    status text NOT NULL,
    unlimited_until timestamp with time zone,
    period_starts_at timestamp with time zone,
    period_ends_at timestamp with time zone,
    total_tokens bigint,
    remaining_tokens bigint,
    total_messages bigint,
    remaining_messages bigint,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_access_key_bundle_bindings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_key_bundle_bindings (
    access_key_id text NOT NULL,
    bundle_id text NOT NULL,
    created_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_access_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_access_keys (
    id text NOT NULL,
    owner_type text NOT NULL,
    owner_id text NOT NULL,
    resolved_project_id text NOT NULL,
    resolved_tenant_id text NOT NULL,
    key_kind text NOT NULL,
    status text NOT NULL,
    public_key_prefix text NOT NULL,
    display_name text NOT NULL,
    external_key text,
    rotated_from_access_key_id text,
    legacy_gateway_api_key_id text,
    legacy_user_credential_id text,
    expires_at timestamp with time zone,
    last_used_at timestamp with time zone,
    metadata jsonb,
    revoked_at timestamp with time zone,
    revoke_reason text,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    secret_material text
);


--
-- Name: gateway_analysis_anomaly_incident_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_analysis_anomaly_incident_history (
    id text NOT NULL,
    incident_id text NOT NULL,
    event_type text NOT NULL,
    actor_user_id text,
    note text,
    metadata jsonb,
    created_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_analysis_anomaly_incidents; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_analysis_anomaly_incidents (
    id text NOT NULL,
    policy_id text,
    fingerprint text NOT NULL,
    project_id text,
    tag text,
    text_mode text,
    code text NOT NULL,
    severity text NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    summary text NOT NULL,
    latest_export_id text,
    previous_export_id text,
    latest_value double precision,
    previous_value double precision,
    delta_value double precision,
    delta_ratio double precision,
    threshold_value double precision,
    first_seen_at timestamp with time zone NOT NULL,
    last_seen_at timestamp with time zone NOT NULL,
    acknowledged_at timestamp with time zone,
    resolved_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    owner_user_id text,
    follow_up_status text DEFAULT 'pending'::text NOT NULL,
    latest_note text,
    resolution_note text,
    last_action_at timestamp with time zone,
    sync_hit_count integer DEFAULT 0 NOT NULL,
    escalation_status text DEFAULT 'none'::text NOT NULL,
    escalated_at timestamp with time zone,
    escalation_reason text,
    last_alert_attempt_at timestamp with time zone,
    last_alerted_at timestamp with time zone,
    last_alert_severity text,
    alert_delivery_count integer DEFAULT 0 NOT NULL,
    route_policy_id text
);


--
-- Name: gateway_analysis_anomaly_policies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_analysis_anomaly_policies (
    id text NOT NULL,
    name text NOT NULL,
    status text DEFAULT 'enabled'::text NOT NULL,
    project_id text,
    tag text,
    text_mode text,
    profile_key text NOT NULL,
    thresholds jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    route_policy_id text,
    auto_sync_enabled boolean DEFAULT false NOT NULL,
    auto_sync_interval_minutes integer,
    last_synced_at timestamp with time zone,
    last_sync_status text,
    last_sync_error text,
    auto_escalate_enabled boolean DEFAULT false NOT NULL,
    escalate_severity_threshold text,
    escalate_after_sync_count integer,
    auto_escalate_owner_user_id text,
    auto_escalate_follow_up_status text,
    alerting_enabled boolean DEFAULT true NOT NULL,
    alert_interval_minutes integer,
    notify_operators_on_escalation boolean DEFAULT true NOT NULL,
    notify_owner_on_escalation boolean DEFAULT true NOT NULL,
    auto_remediation_enabled boolean DEFAULT false NOT NULL,
    auto_remediation_interval_minutes integer,
    auto_remediation_dry_run_first boolean DEFAULT true NOT NULL,
    auto_remediation_action_keys jsonb,
    auto_remediation_max_apply_runs_per_incident integer,
    auto_remediation_require_alert_before_apply boolean DEFAULT false NOT NULL,
    auto_remediation_freeze_on_provider_health_degrade boolean DEFAULT true NOT NULL
);


--
-- Name: gateway_analysis_anomaly_remediation_runs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_analysis_anomaly_remediation_runs (
    id text NOT NULL,
    incident_id text NOT NULL,
    policy_id text,
    route_policy_id text,
    action_key text NOT NULL,
    title text NOT NULL,
    execution_mode text NOT NULL,
    status text NOT NULL,
    dry_run boolean DEFAULT false NOT NULL,
    actor_user_id text NOT NULL,
    note text,
    input jsonb,
    result jsonb,
    before_incident jsonb,
    after_incident jsonb,
    before_route_policy jsonb,
    after_route_policy jsonb,
    error_summary text,
    created_at timestamp with time zone NOT NULL,
    completed_at timestamp with time zone
);


--
-- Name: gateway_analysis_exports; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_analysis_exports (
    id text NOT NULL,
    project_id text,
    label text,
    text_mode text NOT NULL,
    max_text_chars integer NOT NULL,
    filters jsonb NOT NULL,
    object_prefix text NOT NULL,
    manifest_object_key text NOT NULL,
    dataset_object_key text NOT NULL,
    sample_count integer DEFAULT 0 NOT NULL,
    request_artifact_count integer DEFAULT 0 NOT NULL,
    response_artifact_count integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    retention_expires_at timestamp with time zone,
    cleaned_up_at timestamp with time zone,
    last_cleanup_error text
);


--
-- Name: gateway_api_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_api_keys (
    id text NOT NULL,
    project_id text NOT NULL,
    name text NOT NULL,
    status text NOT NULL,
    rotated_from_api_key_id text,
    revoked_at timestamp with time zone,
    revoked_by_user_id text,
    revoke_reason text,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_conversation_archives; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_conversation_archives (
    id text NOT NULL,
    request_audit_id text,
    request_id text NOT NULL,
    project_id text,
    user_id text,
    session_id text,
    provider_account_id text,
    provider_credential_ref text,
    protocol_family text NOT NULL,
    protocol_profile text,
    endpoint_kind text NOT NULL,
    requested_model text,
    resolved_model text,
    status text DEFAULT 'completed'::text NOT NULL,
    upstream_status integer,
    failure_class text,
    failure_scope text,
    request_object_key text,
    response_object_key text,
    redaction_version text DEFAULT 'v1'::text NOT NULL,
    truncated_request boolean DEFAULT false NOT NULL,
    truncated_response boolean DEFAULT false NOT NULL,
    archive_error text,
    retention_expires_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: gateway_conversation_dataset_exports; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_conversation_dataset_exports (
    id text NOT NULL,
    status text DEFAULT 'review_pending'::text NOT NULL,
    filter jsonb DEFAULT '{}'::jsonb NOT NULL,
    sample_size integer,
    row_count integer DEFAULT 0 NOT NULL,
    dataset_object_key text NOT NULL,
    manifest_object_key text NOT NULL,
    created_by text,
    reviewer_id text,
    approval_note text,
    rejected_reason text,
    published_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: gateway_credential_stock_policies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_credential_stock_policies (
    id text NOT NULL,
    stock_class_key text NOT NULL,
    display_name text NOT NULL,
    service_provider_key text NOT NULL,
    implementation_line_key text NOT NULL,
    provider_surface_key text NOT NULL,
    credential_material_kind text NOT NULL,
    provider_account_id text,
    provider_adapter text,
    selector jsonb DEFAULT '{}'::jsonb NOT NULL,
    metric_kind text DEFAULT 'credential_count'::text NOT NULL,
    token_window_key text,
    token_window_seconds bigint,
    min_credential_count integer,
    target_credential_count integer,
    max_credential_count integer,
    min_average_available_tokens bigint,
    target_average_available_tokens bigint,
    signal_enabled boolean DEFAULT true NOT NULL,
    signal_stream text DEFAULT 'gw:credential-stock:signals'::text NOT NULL,
    signal_cooldown_secs bigint DEFAULT 300 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    last_signal_key text,
    last_signal_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT gateway_credential_stock_policies_count_nonnegative_chk CHECK ((((min_credential_count IS NULL) OR (min_credential_count >= 0)) AND ((target_credential_count IS NULL) OR (target_credential_count >= 0)) AND ((max_credential_count IS NULL) OR (max_credential_count >= 0)))),
    CONSTRAINT gateway_credential_stock_policies_metric_kind_chk CHECK ((metric_kind = ANY (ARRAY['credential_count'::text, 'token_window'::text]))),
    CONSTRAINT gateway_credential_stock_policies_token_nonnegative_chk CHECK ((((min_average_available_tokens IS NULL) OR (min_average_available_tokens >= 0)) AND ((target_average_available_tokens IS NULL) OR (target_average_available_tokens >= 0))))
);


--
-- Name: gateway_credential_stock_signal_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_credential_stock_signal_events (
    id text NOT NULL,
    policy_id text NOT NULL,
    stock_class_key text NOT NULL,
    signal_key text NOT NULL,
    severity text NOT NULL,
    stream text NOT NULL,
    payload jsonb NOT NULL,
    published_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: gateway_model_aliases; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_model_aliases (
    id text NOT NULL,
    project_id text,
    alias text NOT NULL,
    provider_account_id text NOT NULL,
    upstream_model text,
    priority integer DEFAULT 100 NOT NULL,
    weight integer DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    scope_type text DEFAULT 'global'::text NOT NULL,
    CONSTRAINT gateway_model_aliases_scope_type_check CHECK ((scope_type = ANY (ARRAY['global'::text, 'provider_special'::text])))
);


--
-- Name: gateway_platform_access_catalog; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_platform_access_catalog (
    id text NOT NULL,
    provider_capability_id text NOT NULL,
    model_code text NOT NULL,
    endpoint_kind text NOT NULL,
    upstream_model text,
    platform_tier text NOT NULL,
    status text NOT NULL,
    operator_weight integer DEFAULT 1 NOT NULL,
    routing_priority integer DEFAULT 100 NOT NULL,
    enabled_for_sale boolean DEFAULT true NOT NULL,
    notes text,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_projects; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_projects (
    id text NOT NULL,
    tenant_id text NOT NULL,
    slug text NOT NULL,
    display_name text NOT NULL,
    status text NOT NULL,
    source_kind text NOT NULL,
    source_key text NOT NULL,
    default_route_policy_id text,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_provider_accounts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_provider_accounts (
    id text NOT NULL,
    label text NOT NULL,
    adapter text NOT NULL,
    protocol_family text NOT NULL,
    status text NOT NULL,
    payload_inline jsonb,
    payload_object_key text,
    payload_content_type text,
    storage_mode text NOT NULL,
    cooldown_until timestamp with time zone,
    last_error text,
    failure_count integer DEFAULT 0 NOT NULL,
    last_health_check_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    archived_at timestamp with time zone,
    execution_mode text DEFAULT 'direct_http'::text NOT NULL,
    endpoint_execution_modes jsonb,
    source_kind text,
    aggregator_api_mode text,
    web_reverse_access_mode text,
    source_notes text,
    service_provider_key text NOT NULL,
    service_provider_label text NOT NULL,
    protocol_profile text NOT NULL
);


--
-- Name: gateway_provider_capability_catalog; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_provider_capability_catalog (
    id text NOT NULL,
    provider_account_id text NOT NULL,
    model_code text NOT NULL,
    endpoint_kind text NOT NULL,
    upstream_model text,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_provider_credential_model_states; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_provider_credential_model_states (
    id text NOT NULL,
    provider_account_id text NOT NULL,
    provider_credential_id text,
    provider_credential_ref text,
    protocol_profile text,
    model text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    failure_class text,
    failure_scope text,
    failure_count integer DEFAULT 0 NOT NULL,
    last_error text,
    last_upstream_status integer,
    cooldown_until timestamp with time zone,
    last_success_at timestamp with time zone,
    last_failure_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: gateway_provider_credentials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_provider_credentials (
    id text NOT NULL,
    provider_account_id text NOT NULL,
    label text NOT NULL,
    status text NOT NULL,
    payload_inline jsonb,
    payload_object_key text,
    payload_content_type text,
    storage_mode text NOT NULL,
    source_kind text DEFAULT 'manual'::text NOT NULL,
    source_path text,
    source_hash text,
    sync_mode text DEFAULT 'manual'::text NOT NULL,
    sync_state text DEFAULT 'idle'::text NOT NULL,
    sync_error text,
    cooldown_until timestamp with time zone,
    last_error text,
    failure_count integer DEFAULT 0 NOT NULL,
    last_health_check_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    archived_at timestamp with time zone
);


--
-- Name: gateway_request_audits; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_request_audits (
    id text NOT NULL,
    project_id text NOT NULL,
    api_key_id text,
    session_id text,
    route_policy_id text,
    provider_account_id text,
    protocol_family text NOT NULL,
    endpoint_kind text NOT NULL,
    requested_model text,
    resolved_model text,
    model_alias text,
    stream boolean DEFAULT false NOT NULL,
    route_attempt_count integer DEFAULT 1 NOT NULL,
    status text NOT NULL,
    upstream_status integer,
    duration_ms integer,
    prompt_tokens integer,
    completion_tokens integer,
    total_tokens integer,
    error_summary text,
    response_id text NOT NULL,
    previous_response_id text,
    client_disconnected_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL,
    completed_at timestamp with time zone,
    updated_at timestamp with time zone NOT NULL,
    route_trace jsonb,
    analysis_profile jsonb,
    request_artifact_object_key text,
    response_artifact_object_key text,
    user_credential_id text,
    cache_creation_input_tokens integer,
    cache_read_input_tokens integer,
    client_has_cache_control boolean DEFAULT false NOT NULL,
    auto_cache_applied boolean DEFAULT false NOT NULL,
    access_key_id text,
    source_access_key_id text,
    CONSTRAINT gateway_request_audits_identity_ck CHECK (((api_key_id IS NOT NULL) OR (user_credential_id IS NOT NULL) OR (access_key_id IS NOT NULL)))
);


--
-- Name: gateway_route_policies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_route_policies (
    id text NOT NULL,
    project_id text NOT NULL,
    name text NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    config jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_schema_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_schema_migrations (
    file_name text NOT NULL,
    applied_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_sessions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_sessions (
    id text NOT NULL,
    project_id text NOT NULL,
    session_key text NOT NULL,
    protocol_family text NOT NULL,
    provider_account_id text NOT NULL,
    latest_response_id text,
    upstream_session_id text,
    runtime_state_object_key text,
    active_request_audit_id text,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    last_used_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone
);


--
-- Name: gateway_tenants; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_tenants (
    id text NOT NULL,
    slug text NOT NULL,
    display_name text NOT NULL,
    status text NOT NULL,
    owner_user_id text,
    source_kind text NOT NULL,
    source_key text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);


--
-- Name: gateway_usage_aggregates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_usage_aggregates (
    bucket_start timestamp with time zone NOT NULL,
    bucket_granularity text DEFAULT '3600s'::text NOT NULL,
    project_id text NOT NULL,
    user_id text NOT NULL,
    provider text NOT NULL,
    provider_credential_ref text NOT NULL,
    model text NOT NULL,
    request_count bigint DEFAULT 0 NOT NULL,
    failure_count bigint DEFAULT 0 NOT NULL,
    prompt_tokens bigint DEFAULT 0 NOT NULL,
    completion_tokens bigint DEFAULT 0 NOT NULL,
    total_tokens bigint DEFAULT 0 NOT NULL,
    cache_creation_input_tokens bigint DEFAULT 0 NOT NULL,
    cache_read_input_tokens bigint DEFAULT 0 NOT NULL,
    latency_ms_sum bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: gateway_user_credentials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.gateway_user_credentials (
    id text NOT NULL,
    user_id text NOT NULL,
    project_id text NOT NULL,
    credential_key text NOT NULL,
    credential_type text NOT NULL,
    status text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    scope jsonb NOT NULL,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone,
    revoked_at timestamp with time zone,
    revoke_reason text
);


--
-- Name: gateway_access_bundle_items gateway_access_bundle_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_bundle_items
    ADD CONSTRAINT gateway_access_bundle_items_pkey PRIMARY KEY (bundle_id, platform_access_id);


--
-- Name: gateway_access_bundles gateway_access_bundles_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_bundles
    ADD CONSTRAINT gateway_access_bundles_pkey PRIMARY KEY (id);


--
-- Name: gateway_access_key_aggregate_memberships gateway_access_key_aggregate_memberships_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_aggregate_memberships
    ADD CONSTRAINT gateway_access_key_aggregate_memberships_pkey PRIMARY KEY (aggregate_access_key_id, member_access_key_id);


--
-- Name: gateway_access_key_balances gateway_access_key_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_balances
    ADD CONSTRAINT gateway_access_key_balances_pkey PRIMARY KEY (access_key_id);


--
-- Name: gateway_access_key_bundle_bindings gateway_access_key_bundle_bindings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_bundle_bindings
    ADD CONSTRAINT gateway_access_key_bundle_bindings_pkey PRIMARY KEY (access_key_id, bundle_id);


--
-- Name: gateway_access_keys gateway_access_keys_legacy_gateway_api_key_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_legacy_gateway_api_key_id_key UNIQUE (legacy_gateway_api_key_id);


--
-- Name: gateway_access_keys gateway_access_keys_legacy_user_credential_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_legacy_user_credential_id_key UNIQUE (legacy_user_credential_id);


--
-- Name: gateway_access_keys gateway_access_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_pkey PRIMARY KEY (id);


--
-- Name: gateway_analysis_anomaly_incident_history gateway_analysis_anomaly_incident_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incident_history
    ADD CONSTRAINT gateway_analysis_anomaly_incident_history_pkey PRIMARY KEY (id);


--
-- Name: gateway_analysis_anomaly_incidents gateway_analysis_anomaly_incidents_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incidents
    ADD CONSTRAINT gateway_analysis_anomaly_incidents_pkey PRIMARY KEY (id);


--
-- Name: gateway_analysis_anomaly_policies gateway_analysis_anomaly_policies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_policies
    ADD CONSTRAINT gateway_analysis_anomaly_policies_pkey PRIMARY KEY (id);


--
-- Name: gateway_analysis_anomaly_remediation_runs gateway_analysis_anomaly_remediation_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_remediation_runs
    ADD CONSTRAINT gateway_analysis_anomaly_remediation_runs_pkey PRIMARY KEY (id);


--
-- Name: gateway_analysis_exports gateway_analysis_exports_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_exports
    ADD CONSTRAINT gateway_analysis_exports_pkey PRIMARY KEY (id);


--
-- Name: gateway_api_keys gateway_api_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_api_keys
    ADD CONSTRAINT gateway_api_keys_pkey PRIMARY KEY (id);


--
-- Name: gateway_conversation_archives gateway_conversation_archives_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_conversation_archives
    ADD CONSTRAINT gateway_conversation_archives_pkey PRIMARY KEY (id);


--
-- Name: gateway_conversation_dataset_exports gateway_conversation_dataset_exports_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_conversation_dataset_exports
    ADD CONSTRAINT gateway_conversation_dataset_exports_pkey PRIMARY KEY (id);


--
-- Name: gateway_credential_stock_policies gateway_credential_stock_policies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_credential_stock_policies
    ADD CONSTRAINT gateway_credential_stock_policies_pkey PRIMARY KEY (id);


--
-- Name: gateway_credential_stock_policies gateway_credential_stock_policies_stock_class_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_credential_stock_policies
    ADD CONSTRAINT gateway_credential_stock_policies_stock_class_key_key UNIQUE (stock_class_key);


--
-- Name: gateway_credential_stock_signal_events gateway_credential_stock_signal_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_credential_stock_signal_events
    ADD CONSTRAINT gateway_credential_stock_signal_events_pkey PRIMARY KEY (id);


--
-- Name: gateway_model_aliases gateway_model_aliases_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_model_aliases
    ADD CONSTRAINT gateway_model_aliases_pkey PRIMARY KEY (id);


--
-- Name: gateway_platform_access_catalog gateway_platform_access_catalog_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_platform_access_catalog
    ADD CONSTRAINT gateway_platform_access_catalog_pkey PRIMARY KEY (id);


--
-- Name: gateway_projects gateway_projects_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_projects
    ADD CONSTRAINT gateway_projects_pkey PRIMARY KEY (id);


--
-- Name: gateway_provider_accounts gateway_provider_accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_accounts
    ADD CONSTRAINT gateway_provider_accounts_pkey PRIMARY KEY (id);


--
-- Name: gateway_provider_capability_catalog gateway_provider_capability_catalog_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_capability_catalog
    ADD CONSTRAINT gateway_provider_capability_catalog_pkey PRIMARY KEY (id);


--
-- Name: gateway_provider_credential_model_states gateway_provider_credential_m_provider_account_id_provider__key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credential_model_states
    ADD CONSTRAINT gateway_provider_credential_m_provider_account_id_provider__key UNIQUE (provider_account_id, provider_credential_id, provider_credential_ref, protocol_profile, model);


--
-- Name: gateway_provider_credential_model_states gateway_provider_credential_model_states_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credential_model_states
    ADD CONSTRAINT gateway_provider_credential_model_states_pkey PRIMARY KEY (id);


--
-- Name: gateway_provider_credentials gateway_provider_credentials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credentials
    ADD CONSTRAINT gateway_provider_credentials_pkey PRIMARY KEY (id);


--
-- Name: gateway_request_audits gateway_request_audits_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_pkey PRIMARY KEY (id);


--
-- Name: gateway_route_policies gateway_route_policies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_route_policies
    ADD CONSTRAINT gateway_route_policies_pkey PRIMARY KEY (id);


--
-- Name: gateway_schema_migrations gateway_schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_schema_migrations
    ADD CONSTRAINT gateway_schema_migrations_pkey PRIMARY KEY (file_name);


--
-- Name: gateway_sessions gateway_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_sessions
    ADD CONSTRAINT gateway_sessions_pkey PRIMARY KEY (id);


--
-- Name: gateway_tenants gateway_tenants_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_tenants
    ADD CONSTRAINT gateway_tenants_pkey PRIMARY KEY (id);


--
-- Name: gateway_usage_aggregates gateway_usage_aggregates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_usage_aggregates
    ADD CONSTRAINT gateway_usage_aggregates_pkey PRIMARY KEY (bucket_start, bucket_granularity, project_id, user_id, provider, provider_credential_ref, model);


--
-- Name: gateway_user_credentials gateway_user_credentials_credential_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_user_credentials
    ADD CONSTRAINT gateway_user_credentials_credential_key_key UNIQUE (credential_key);


--
-- Name: gateway_user_credentials gateway_user_credentials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_user_credentials
    ADD CONSTRAINT gateway_user_credentials_pkey PRIMARY KEY (id);


--
-- Name: gateway_access_bundles_slug_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_access_bundles_slug_idx ON public.gateway_access_bundles USING btree (slug);


--
-- Name: gateway_access_key_aggregate_memberships_member_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_access_key_aggregate_memberships_member_idx ON public.gateway_access_key_aggregate_memberships USING btree (member_access_key_id);


--
-- Name: gateway_access_key_balances_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_access_key_balances_status_idx ON public.gateway_access_key_balances USING btree (status, balance_mode);


--
-- Name: gateway_access_keys_external_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_access_keys_external_key_idx ON public.gateway_access_keys USING btree (external_key) WHERE (external_key IS NOT NULL);


--
-- Name: gateway_access_keys_owner_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_access_keys_owner_status_idx ON public.gateway_access_keys USING btree (owner_type, owner_id, status);


--
-- Name: gateway_access_keys_project_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_access_keys_project_status_idx ON public.gateway_access_keys USING btree (resolved_project_id, status);


--
-- Name: gateway_analysis_anomaly_incident_history_incident_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incident_history_incident_created_idx ON public.gateway_analysis_anomaly_incident_history USING btree (incident_id, created_at);


--
-- Name: gateway_analysis_anomaly_incidents_code_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_code_status_idx ON public.gateway_analysis_anomaly_incidents USING btree (code, status);


--
-- Name: gateway_analysis_anomaly_incidents_escalation_alert_attempt_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_escalation_alert_attempt_idx ON public.gateway_analysis_anomaly_incidents USING btree (escalation_status, status, last_alert_attempt_at);


--
-- Name: gateway_analysis_anomaly_incidents_escalation_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_escalation_status_idx ON public.gateway_analysis_anomaly_incidents USING btree (escalation_status, status);


--
-- Name: gateway_analysis_anomaly_incidents_fingerprint_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_analysis_anomaly_incidents_fingerprint_idx ON public.gateway_analysis_anomaly_incidents USING btree (fingerprint);


--
-- Name: gateway_analysis_anomaly_incidents_owner_follow_up_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_owner_follow_up_idx ON public.gateway_analysis_anomaly_incidents USING btree (owner_user_id, follow_up_status);


--
-- Name: gateway_analysis_anomaly_incidents_project_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_project_status_idx ON public.gateway_analysis_anomaly_incidents USING btree (project_id, status);


--
-- Name: gateway_analysis_anomaly_incidents_route_policy_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_route_policy_status_idx ON public.gateway_analysis_anomaly_incidents USING btree (route_policy_id, status);


--
-- Name: gateway_analysis_anomaly_incidents_status_severity_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_incidents_status_severity_idx ON public.gateway_analysis_anomaly_incidents USING btree (status, severity);


--
-- Name: gateway_analysis_anomaly_policies_alerting_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_alerting_status_idx ON public.gateway_analysis_anomaly_policies USING btree (alerting_enabled, status);


--
-- Name: gateway_analysis_anomaly_policies_auto_escalate_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_auto_escalate_status_idx ON public.gateway_analysis_anomaly_policies USING btree (auto_escalate_enabled, status);


--
-- Name: gateway_analysis_anomaly_policies_auto_remediation_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_auto_remediation_status_idx ON public.gateway_analysis_anomaly_policies USING btree (auto_remediation_enabled, status);


--
-- Name: gateway_analysis_anomaly_policies_project_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_project_status_idx ON public.gateway_analysis_anomaly_policies USING btree (project_id, status);


--
-- Name: gateway_analysis_anomaly_policies_route_policy_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_route_policy_status_idx ON public.gateway_analysis_anomaly_policies USING btree (route_policy_id, status);


--
-- Name: gateway_analysis_anomaly_policies_status_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_policies_status_created_idx ON public.gateway_analysis_anomaly_policies USING btree (status, created_at);


--
-- Name: gateway_analysis_anomaly_remediation_runs_action_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_remediation_runs_action_status_idx ON public.gateway_analysis_anomaly_remediation_runs USING btree (action_key, status, created_at);


--
-- Name: gateway_analysis_anomaly_remediation_runs_incident_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_remediation_runs_incident_created_idx ON public.gateway_analysis_anomaly_remediation_runs USING btree (incident_id, created_at);


--
-- Name: gateway_analysis_anomaly_remediation_runs_route_policy_created_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_anomaly_remediation_runs_route_policy_created_ ON public.gateway_analysis_anomaly_remediation_runs USING btree (route_policy_id, created_at);


--
-- Name: gateway_analysis_exports_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_exports_created_idx ON public.gateway_analysis_exports USING btree (created_at);


--
-- Name: gateway_analysis_exports_project_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_exports_project_created_idx ON public.gateway_analysis_exports USING btree (project_id, created_at);


--
-- Name: gateway_analysis_exports_status_retention_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_exports_status_retention_idx ON public.gateway_analysis_exports USING btree (status, retention_expires_at);


--
-- Name: gateway_analysis_exports_text_mode_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_analysis_exports_text_mode_created_idx ON public.gateway_analysis_exports USING btree (text_mode, created_at);


--
-- Name: gateway_api_keys_project_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_api_keys_project_status_idx ON public.gateway_api_keys USING btree (project_id, status);


--
-- Name: gateway_model_aliases_alias_enabled_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_model_aliases_alias_enabled_idx ON public.gateway_model_aliases USING btree (alias, enabled);


--
-- Name: gateway_model_aliases_project_alias_enabled_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_model_aliases_project_alias_enabled_idx ON public.gateway_model_aliases USING btree (project_id, alias, enabled);


--
-- Name: gateway_platform_access_catalog_model_endpoint_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_platform_access_catalog_model_endpoint_idx ON public.gateway_platform_access_catalog USING btree (model_code, endpoint_kind);


--
-- Name: gateway_platform_access_catalog_status_sale_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_platform_access_catalog_status_sale_idx ON public.gateway_platform_access_catalog USING btree (status, enabled_for_sale);


--
-- Name: gateway_projects_source_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_projects_source_key_idx ON public.gateway_projects USING btree (source_kind, source_key);


--
-- Name: gateway_provider_accounts_protocol_profile_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_accounts_protocol_profile_idx ON public.gateway_provider_accounts USING btree (protocol_profile);


--
-- Name: gateway_provider_accounts_protocol_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_accounts_protocol_status_idx ON public.gateway_provider_accounts USING btree (protocol_family, status);


--
-- Name: gateway_provider_accounts_service_provider_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_accounts_service_provider_key_idx ON public.gateway_provider_accounts USING btree (service_provider_key);


--
-- Name: gateway_provider_accounts_source_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_accounts_source_status_idx ON public.gateway_provider_accounts USING btree (source_kind, status);


--
-- Name: gateway_provider_accounts_status_cooldown_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_accounts_status_cooldown_idx ON public.gateway_provider_accounts USING btree (status, cooldown_until);


--
-- Name: gateway_provider_capability_catalog_provider_model_endpoint_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_provider_capability_catalog_provider_model_endpoint_idx ON public.gateway_provider_capability_catalog USING btree (provider_account_id, model_code, endpoint_kind);


--
-- Name: gateway_provider_credentials_provider_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_credentials_provider_status_idx ON public.gateway_provider_credentials USING btree (provider_account_id, status);


--
-- Name: gateway_provider_credentials_source_path_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_provider_credentials_source_path_idx ON public.gateway_provider_credentials USING btree (source_path) WHERE (source_path IS NOT NULL);


--
-- Name: gateway_provider_credentials_status_cooldown_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_provider_credentials_status_cooldown_idx ON public.gateway_provider_credentials USING btree (status, cooldown_until);


--
-- Name: gateway_request_audits_access_key_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_access_key_created_idx ON public.gateway_request_audits USING btree (access_key_id, created_at);


--
-- Name: gateway_request_audits_project_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_project_created_idx ON public.gateway_request_audits USING btree (project_id, created_at DESC);


--
-- Name: gateway_request_audits_provider_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_provider_created_idx ON public.gateway_request_audits USING btree (provider_account_id, created_at DESC);


--
-- Name: gateway_request_audits_provider_inventory_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_provider_inventory_idx ON public.gateway_request_audits USING btree (provider_account_id, created_at DESC) INCLUDE (status, prompt_tokens, completion_tokens, total_tokens, cache_creation_input_tokens, cache_read_input_tokens, requested_model, resolved_model, model_alias) WHERE (provider_account_id IS NOT NULL);


--
-- Name: gateway_request_audits_response_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_request_audits_response_id_idx ON public.gateway_request_audits USING btree (response_id);


--
-- Name: gateway_request_audits_source_access_key_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_source_access_key_created_idx ON public.gateway_request_audits USING btree (source_access_key_id, created_at);


--
-- Name: gateway_request_audits_user_credential_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_request_audits_user_credential_created_idx ON public.gateway_request_audits USING btree (user_credential_id, created_at DESC);


--
-- Name: gateway_route_policies_project_default_enabled_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_route_policies_project_default_enabled_idx ON public.gateway_route_policies USING btree (project_id, is_default, enabled);


--
-- Name: gateway_sessions_project_last_used_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_sessions_project_last_used_idx ON public.gateway_sessions USING btree (project_id, last_used_at DESC);


--
-- Name: gateway_sessions_project_session_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_sessions_project_session_idx ON public.gateway_sessions USING btree (project_id, session_key);


--
-- Name: gateway_tenants_source_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX gateway_tenants_source_key_idx ON public.gateway_tenants USING btree (source_kind, source_key);


--
-- Name: gateway_user_credentials_credential_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_user_credentials_credential_key_idx ON public.gateway_user_credentials USING btree (credential_key);


--
-- Name: gateway_user_credentials_project_id_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_user_credentials_project_id_status_idx ON public.gateway_user_credentials USING btree (project_id, status);


--
-- Name: gateway_user_credentials_status_expires_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_user_credentials_status_expires_at_idx ON public.gateway_user_credentials USING btree (status, expires_at);


--
-- Name: gateway_user_credentials_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gateway_user_credentials_user_id_idx ON public.gateway_user_credentials USING btree (user_id);


--
-- Name: idx_gateway_conversation_archives_project_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_archives_project_created ON public.gateway_conversation_archives USING btree (project_id, created_at DESC);


--
-- Name: idx_gateway_conversation_archives_provider_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_archives_provider_created ON public.gateway_conversation_archives USING btree (provider_account_id, created_at DESC);


--
-- Name: idx_gateway_conversation_archives_retention; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_archives_retention ON public.gateway_conversation_archives USING btree (retention_expires_at);


--
-- Name: idx_gateway_conversation_archives_status_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_archives_status_created ON public.gateway_conversation_archives USING btree (status, created_at DESC);


--
-- Name: idx_gateway_conversation_archives_user_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_archives_user_created ON public.gateway_conversation_archives USING btree (user_id, created_at DESC);


--
-- Name: idx_gateway_conversation_dataset_exports_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_conversation_dataset_exports_status ON public.gateway_conversation_dataset_exports USING btree (status, created_at DESC);


--
-- Name: idx_gateway_credential_stock_policies_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_policies_enabled ON public.gateway_credential_stock_policies USING btree (enabled, stock_class_key);


--
-- Name: idx_gateway_credential_stock_policies_provider_account; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_policies_provider_account ON public.gateway_credential_stock_policies USING btree (provider_account_id) WHERE (provider_account_id IS NOT NULL);


--
-- Name: idx_gateway_credential_stock_policies_provider_line; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_policies_provider_line ON public.gateway_credential_stock_policies USING btree (service_provider_key, implementation_line_key, provider_surface_key, credential_material_kind);


--
-- Name: idx_gateway_credential_stock_signal_events_policy_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_signal_events_policy_created ON public.gateway_credential_stock_signal_events USING btree (policy_id, created_at DESC);


--
-- Name: idx_gateway_credential_stock_signal_events_signal_key; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_signal_events_signal_key ON public.gateway_credential_stock_signal_events USING btree (signal_key, created_at DESC);


--
-- Name: idx_gateway_credential_stock_signal_events_stock_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_credential_stock_signal_events_stock_created ON public.gateway_credential_stock_signal_events USING btree (stock_class_key, created_at DESC);


--
-- Name: idx_gateway_provider_credential_model_states_cooldown; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_provider_credential_model_states_cooldown ON public.gateway_provider_credential_model_states USING btree (status, cooldown_until);


--
-- Name: idx_gateway_provider_credential_model_states_credential_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_provider_credential_model_states_credential_model ON public.gateway_provider_credential_model_states USING btree (provider_credential_id, protocol_profile, model);


--
-- Name: idx_gateway_provider_credential_model_states_credential_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_provider_credential_model_states_credential_status ON public.gateway_provider_credential_model_states USING btree (provider_credential_id, status);


--
-- Name: idx_gateway_provider_credential_model_states_provider_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_provider_credential_model_states_provider_status ON public.gateway_provider_credential_model_states USING btree (provider_account_id, status);


--
-- Name: idx_gateway_usage_aggregates_project_bucket; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_usage_aggregates_project_bucket ON public.gateway_usage_aggregates USING btree (project_id, bucket_start DESC);


--
-- Name: idx_gateway_usage_aggregates_user_credential_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gateway_usage_aggregates_user_credential_model ON public.gateway_usage_aggregates USING btree (user_id, provider_credential_ref, model, bucket_start DESC);


--
-- Name: gateway_access_bundle_items gateway_access_bundle_items_bundle_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_bundle_items
    ADD CONSTRAINT gateway_access_bundle_items_bundle_id_fkey FOREIGN KEY (bundle_id) REFERENCES public.gateway_access_bundles(id) ON DELETE CASCADE;


--
-- Name: gateway_access_bundle_items gateway_access_bundle_items_platform_access_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_bundle_items
    ADD CONSTRAINT gateway_access_bundle_items_platform_access_id_fkey FOREIGN KEY (platform_access_id) REFERENCES public.gateway_platform_access_catalog(id) ON DELETE CASCADE;


--
-- Name: gateway_access_bundles gateway_access_bundles_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_bundles
    ADD CONSTRAINT gateway_access_bundles_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE SET NULL;


--
-- Name: gateway_access_key_aggregate_memberships gateway_access_key_aggregate_membe_aggregate_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_aggregate_memberships
    ADD CONSTRAINT gateway_access_key_aggregate_membe_aggregate_access_key_id_fkey FOREIGN KEY (aggregate_access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE CASCADE;


--
-- Name: gateway_access_key_aggregate_memberships gateway_access_key_aggregate_membersh_member_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_aggregate_memberships
    ADD CONSTRAINT gateway_access_key_aggregate_membersh_member_access_key_id_fkey FOREIGN KEY (member_access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE CASCADE;


--
-- Name: gateway_access_key_balances gateway_access_key_balances_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_balances
    ADD CONSTRAINT gateway_access_key_balances_access_key_id_fkey FOREIGN KEY (access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE CASCADE;


--
-- Name: gateway_access_key_bundle_bindings gateway_access_key_bundle_bindings_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_bundle_bindings
    ADD CONSTRAINT gateway_access_key_bundle_bindings_access_key_id_fkey FOREIGN KEY (access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE CASCADE;


--
-- Name: gateway_access_key_bundle_bindings gateway_access_key_bundle_bindings_bundle_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_key_bundle_bindings
    ADD CONSTRAINT gateway_access_key_bundle_bindings_bundle_id_fkey FOREIGN KEY (bundle_id) REFERENCES public.gateway_access_bundles(id) ON DELETE CASCADE;


--
-- Name: gateway_access_keys gateway_access_keys_resolved_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_resolved_project_id_fkey FOREIGN KEY (resolved_project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_access_keys gateway_access_keys_resolved_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_resolved_tenant_id_fkey FOREIGN KEY (resolved_tenant_id) REFERENCES public.gateway_tenants(id) ON DELETE CASCADE;


--
-- Name: gateway_access_keys gateway_access_keys_rotated_from_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_access_keys
    ADD CONSTRAINT gateway_access_keys_rotated_from_access_key_id_fkey FOREIGN KEY (rotated_from_access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_incident_history gateway_analysis_anomaly_incident_history_incident_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incident_history
    ADD CONSTRAINT gateway_analysis_anomaly_incident_history_incident_id_fkey FOREIGN KEY (incident_id) REFERENCES public.gateway_analysis_anomaly_incidents(id) ON DELETE CASCADE;


--
-- Name: gateway_analysis_anomaly_incidents gateway_analysis_anomaly_incidents_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incidents
    ADD CONSTRAINT gateway_analysis_anomaly_incidents_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.gateway_analysis_anomaly_policies(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_incidents gateway_analysis_anomaly_incidents_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incidents
    ADD CONSTRAINT gateway_analysis_anomaly_incidents_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_incidents gateway_analysis_anomaly_incidents_route_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_incidents
    ADD CONSTRAINT gateway_analysis_anomaly_incidents_route_policy_id_fkey FOREIGN KEY (route_policy_id) REFERENCES public.gateway_route_policies(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_policies gateway_analysis_anomaly_policies_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_policies
    ADD CONSTRAINT gateway_analysis_anomaly_policies_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_policies gateway_analysis_anomaly_policies_route_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_policies
    ADD CONSTRAINT gateway_analysis_anomaly_policies_route_policy_id_fkey FOREIGN KEY (route_policy_id) REFERENCES public.gateway_route_policies(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_remediation_runs gateway_analysis_anomaly_remediation_runs_incident_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_remediation_runs
    ADD CONSTRAINT gateway_analysis_anomaly_remediation_runs_incident_id_fkey FOREIGN KEY (incident_id) REFERENCES public.gateway_analysis_anomaly_incidents(id) ON DELETE CASCADE;


--
-- Name: gateway_analysis_anomaly_remediation_runs gateway_analysis_anomaly_remediation_runs_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_remediation_runs
    ADD CONSTRAINT gateway_analysis_anomaly_remediation_runs_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.gateway_analysis_anomaly_policies(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_anomaly_remediation_runs gateway_analysis_anomaly_remediation_runs_route_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_anomaly_remediation_runs
    ADD CONSTRAINT gateway_analysis_anomaly_remediation_runs_route_policy_id_fkey FOREIGN KEY (route_policy_id) REFERENCES public.gateway_route_policies(id) ON DELETE SET NULL;


--
-- Name: gateway_analysis_exports gateway_analysis_exports_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_analysis_exports
    ADD CONSTRAINT gateway_analysis_exports_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE SET NULL;


--
-- Name: gateway_api_keys gateway_api_keys_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_api_keys
    ADD CONSTRAINT gateway_api_keys_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_conversation_archives gateway_conversation_archives_request_audit_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_conversation_archives
    ADD CONSTRAINT gateway_conversation_archives_request_audit_id_fkey FOREIGN KEY (request_audit_id) REFERENCES public.gateway_request_audits(id) ON DELETE SET NULL;


--
-- Name: gateway_credential_stock_policies gateway_credential_stock_policies_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_credential_stock_policies
    ADD CONSTRAINT gateway_credential_stock_policies_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE SET NULL;


--
-- Name: gateway_credential_stock_signal_events gateway_credential_stock_signal_events_policy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_credential_stock_signal_events
    ADD CONSTRAINT gateway_credential_stock_signal_events_policy_id_fkey FOREIGN KEY (policy_id) REFERENCES public.gateway_credential_stock_policies(id) ON DELETE CASCADE;


--
-- Name: gateway_model_aliases gateway_model_aliases_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_model_aliases
    ADD CONSTRAINT gateway_model_aliases_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_model_aliases gateway_model_aliases_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_model_aliases
    ADD CONSTRAINT gateway_model_aliases_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE CASCADE;


--
-- Name: gateway_platform_access_catalog gateway_platform_access_catalog_provider_capability_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_platform_access_catalog
    ADD CONSTRAINT gateway_platform_access_catalog_provider_capability_id_fkey FOREIGN KEY (provider_capability_id) REFERENCES public.gateway_provider_capability_catalog(id) ON DELETE CASCADE;


--
-- Name: gateway_projects gateway_projects_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_projects
    ADD CONSTRAINT gateway_projects_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.gateway_tenants(id) ON DELETE CASCADE;


--
-- Name: gateway_provider_capability_catalog gateway_provider_capability_catalog_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_capability_catalog
    ADD CONSTRAINT gateway_provider_capability_catalog_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE CASCADE;


--
-- Name: gateway_provider_credential_model_states gateway_provider_credential_model_s_provider_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credential_model_states
    ADD CONSTRAINT gateway_provider_credential_model_s_provider_credential_id_fkey FOREIGN KEY (provider_credential_id) REFERENCES public.gateway_provider_credentials(id) ON DELETE CASCADE;


--
-- Name: gateway_provider_credential_model_states gateway_provider_credential_model_stat_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credential_model_states
    ADD CONSTRAINT gateway_provider_credential_model_stat_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE CASCADE;


--
-- Name: gateway_provider_credentials gateway_provider_credentials_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_provider_credentials
    ADD CONSTRAINT gateway_provider_credentials_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE CASCADE;


--
-- Name: gateway_request_audits gateway_request_audits_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_access_key_id_fkey FOREIGN KEY (access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE SET NULL;


--
-- Name: gateway_request_audits gateway_request_audits_api_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_api_key_id_fkey FOREIGN KEY (api_key_id) REFERENCES public.gateway_api_keys(id) ON DELETE CASCADE;


--
-- Name: gateway_request_audits gateway_request_audits_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_request_audits gateway_request_audits_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE SET NULL;


--
-- Name: gateway_request_audits gateway_request_audits_session_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_session_id_fkey FOREIGN KEY (session_id) REFERENCES public.gateway_sessions(id) ON DELETE SET NULL;


--
-- Name: gateway_request_audits gateway_request_audits_source_access_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_source_access_key_id_fkey FOREIGN KEY (source_access_key_id) REFERENCES public.gateway_access_keys(id) ON DELETE SET NULL;


--
-- Name: gateway_request_audits gateway_request_audits_user_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_request_audits
    ADD CONSTRAINT gateway_request_audits_user_credential_id_fkey FOREIGN KEY (user_credential_id) REFERENCES public.gateway_user_credentials(id) ON DELETE SET NULL;


--
-- Name: gateway_route_policies gateway_route_policies_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_route_policies
    ADD CONSTRAINT gateway_route_policies_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_sessions gateway_sessions_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_sessions
    ADD CONSTRAINT gateway_sessions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- Name: gateway_sessions gateway_sessions_provider_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_sessions
    ADD CONSTRAINT gateway_sessions_provider_account_id_fkey FOREIGN KEY (provider_account_id) REFERENCES public.gateway_provider_accounts(id) ON DELETE CASCADE;


--
-- Name: gateway_user_credentials gateway_user_credentials_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.gateway_user_credentials
    ADD CONSTRAINT gateway_user_credentials_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.gateway_projects(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--


--
-- PostgreSQL database dump
--


-- Dumped from database version 16.14 (Debian 16.14-1.pgdg12+1)
-- Dumped by pg_dump version 16.14 (Debian 16.14-1.pgdg12+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: gateway_schema_migrations; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.gateway_schema_migrations (file_name, applied_at) FROM stdin;
20260405_00_ai_gateway_bootstrap.sql	2026-04-12 16:47:58.984289+00
20260406_00_ai_gateway_runtime_hardening.sql	2026-04-12 16:47:59.055044+00
20260406_01_ai_gateway_route_trace_and_provider_limits.sql	2026-04-12 16:47:59.090412+00
20260406_02_ai_gateway_request_artifacts.sql	2026-04-12 16:47:59.095226+00
20260406_03_ai_gateway_analysis_exports.sql	2026-04-12 16:47:59.099857+00
20260406_04_ai_gateway_analysis_export_lifecycle.sql	2026-04-12 16:47:59.117982+00
20260406_05_ai_gateway_analysis_anomaly_governance.sql	2026-04-12 16:47:59.124671+00
20260406_06_ai_gateway_analysis_anomaly_incident_followup.sql	2026-04-12 16:47:59.158233+00
20260406_07_ai_gateway_analysis_anomaly_policy_link_and_sync.sql	2026-04-12 16:47:59.165156+00
20260406_08_ai_gateway_analysis_anomaly_incident_history.sql	2026-04-12 16:47:59.172747+00
20260406_09_ai_gateway_analysis_anomaly_escalation.sql	2026-04-12 16:47:59.184981+00
20260406_10_ai_gateway_analysis_anomaly_alerts.sql	2026-04-12 16:47:59.19491+00
20260406_11_ai_gateway_analysis_anomaly_remediation_runs.sql	2026-04-12 16:47:59.203399+00
20260406_12_ai_gateway_analysis_anomaly_auto_remediation.sql	2026-04-12 16:47:59.226427+00
20260406_13_ai_gateway_analysis_anomaly_auto_remediation_guardrails.sql	2026-04-12 16:47:59.232976+00
20260407_00_ai_gateway_analysis_anomaly_incident_route_policy.sql	2026-04-12 16:47:59.236389+00
20260411_00_ai_gateway_execution_mode_and_browser_executor.sql	2026-04-12 16:47:59.243862+00
20260411_01_ai_gateway_provider_source_profile.sql	2026-04-12 16:47:59.248721+00
20260413_00_gateway_user_credentials.sql	2026-04-12 16:47:59.254257+00
20260413_01_gateway_request_audits_user_credentials.sql	2026-04-14 09:40:24.353943+00
20260414_02_gateway_request_audits_cache_tokens.sql	2026-04-14 09:40:24.396173+00
20260414_03_gateway_request_audits_prompt_cache_telemetry.sql	2026-04-14 09:40:24.402049+00
20260414_04_gateway_access_system.sql	2026-04-14 21:00:45.915314+00
20260415_00_gateway_provider_credentials.sql	2026-04-15 10:42:02.082229+00
20260416_00_gateway_model_alias_scope.sql	2026-04-16 08:39:32.590987+00
20260416_01_gateway_access_bundle_billing_mode.sql	2026-04-17 00:35:27.002738+00
20260418_00_gateway_provider_service_identity.sql	2026-04-18 11:05:43.270997+00
20260418_01_gateway_request_audits_access_keys.sql	2026-04-19 18:46:43.827868+00
20260419_00_gateway_provider_protocol_profile.sql	2026-04-19 18:46:43.877581+00
20260530_01_gateway_conversation_archives.sql	2026-06-17 03:27:12.111812+00
20260530_02_gateway_mature_phase_state_usage_datasets.sql	2026-06-17 03:27:12.397957+00
20260530_03_gateway_credential_stock.sql	2026-06-17 03:27:12.606545+00
20260617_00_gateway_provider_inventory_indexes.sql	2026-06-17 07:35:50.569245+00
\.


--
-- PostgreSQL database dump complete
--


