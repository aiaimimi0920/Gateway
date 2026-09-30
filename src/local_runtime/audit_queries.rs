//! Bound query parameters; filter before limiting, with the same console DTOs.
use super::{audits::decode, storage_error, LocalRuntime};
use crate::{
    db::{GatewayRequestAuditView, RequestAuditFilters},
    error::GatewayError,
};
use sqlx::{QueryBuilder, Sqlite};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

impl LocalRuntime {
    pub async fn list_audits(
        &self,
        filters: &RequestAuditFilters,
    ) -> Result<Vec<GatewayRequestAuditView>, GatewayError> {
        let mut query =
            QueryBuilder::<Sqlite>::new("SELECT payload FROM request_audits WHERE 1 = 1");
        for (field, value) in [
            ("projectId", &filters.project_id),
            ("routePolicyId", &filters.route_policy_id),
            ("providerAccountId", &filters.provider_account_id),
            ("sessionId", &filters.session_id),
            ("apiKeyId", &filters.api_key_id),
            ("userCredentialId", &filters.user_credential_id),
            ("accessKeyId", &filters.access_key_id),
            ("sourceAccessKeyId", &filters.source_access_key_id),
            ("responseId", &filters.response_id),
            ("protocolFamily", &filters.protocol_family),
            ("status", &filters.status),
            ("endpointKind", &filters.endpoint_kind),
            ("routeTrace.errorCode", &filters.error_code),
        ] {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                query
                    .push(" AND json_extract(payload, '$.")
                    .push(field)
                    .push("') = ")
                    .push_bind(value.to_owned());
            }
        }
        for (field, value) in [
            ("stream", filters.stream),
            ("routeTrace.fallbackEligible", filters.fallback_eligible),
        ] {
            if let Some(value) = value {
                query
                    .push(" AND coalesce(json_extract(payload, '$.")
                    .push(field)
                    .push("'), 0) = ")
                    .push_bind(value);
            }
        }
        if let Some(value) = filters.artifact_available {
            query
                .push(
                    " AND (json_extract(payload, '$.requestArtifactObjectKey') IS NOT NULL OR
                json_extract(payload, '$.responseArtifactObjectKey') IS NOT NULL) = ",
                )
                .push_bind(value);
        }
        let from = parse_time(filters.created_from.as_deref())?;
        let to = parse_time(filters.created_to.as_deref())?;
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(GatewayError::bad_request(
                "createdFrom cannot exceed createdTo",
            ));
        }
        for (operator, time) in [(" >= ", from), (" <= ", to)] {
            if let Some(time) = time {
                query
                    .push(" AND julianday(created)")
                    .push(operator)
                    .push("julianday(")
                    .push_bind(time.format(&Rfc3339).expect("timestamp"))
                    .push(")");
            }
        }
        query
            .push(" ORDER BY created DESC, id DESC LIMIT ")
            .push_bind(filters.limit.unwrap_or(200).clamp(1, 1000) as i64);
        let payloads: Vec<String> = query
            .build_query_scalar()
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        payloads.iter().map(|payload| decode(payload)).collect()
    }
}

fn parse_time(value: Option<&str>) -> Result<Option<OffsetDateTime>, GatewayError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339)
                .map_err(|_| GatewayError::bad_request("Invalid RFC3339 audit time filter"))
        })
        .transpose()
}
