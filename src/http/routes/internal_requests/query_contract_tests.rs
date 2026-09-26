//! Wire selectors must survive projection into the database's admission filters.

use super::anomaly_incidents::{into_incident_filters, AnomalyIncidentQuery};
use super::audits::{into_filters, RequestAuditQuery};
use super::management_actor_user_id;
use super::remediation::{into_remediation_queue_filters, RemediationQueueQuery};
use axum::extract::Query;
use axum::http::{HeaderMap as AxumHeaderMap, HeaderValue, Uri};

#[test]
fn audit_query_preserves_distinct_access_identities_and_explicit_false() {
    let uri: Uri = concat!(
        "/?projectId=project-a&routePolicyId=route-a&providerAccountId=provider-a",
        "&apiKeyId=api-a&userCredentialId=credential-a&accessKeyId=access-a",
        "&sourceAccessKeyId=source-a&stream=false&fallbackEligible=false",
        "&artifactAvailable=false&limit=0"
    )
    .parse()
    .unwrap();
    let Query(query) = Query::<RequestAuditQuery>::try_from_uri(&uri).unwrap();
    let filters = into_filters(query);
    assert_eq!(filters.project_id.as_deref(), Some("project-a"));
    assert_eq!(filters.route_policy_id.as_deref(), Some("route-a"));
    assert_eq!(filters.provider_account_id.as_deref(), Some("provider-a"));
    assert_eq!(filters.api_key_id.as_deref(), Some("api-a"));
    assert_eq!(filters.user_credential_id.as_deref(), Some("credential-a"));
    assert_eq!(filters.access_key_id.as_deref(), Some("access-a"));
    assert_eq!(filters.source_access_key_id.as_deref(), Some("source-a"));
    assert_eq!(filters.stream, Some(false));
    assert_eq!(filters.fallback_eligible, Some(false));
    assert_eq!(filters.artifact_available, Some(false));
    assert_eq!(filters.limit, Some(0));
}

#[test]
fn incident_and_remediation_queue_keep_the_same_owner_boundary() {
    let uri: Uri = concat!(
        "/?incidentId=incident-a&projectId=project-a&routePolicyId=route-a",
        "&ownerUserId=owner-a&dueOnly=false&limit=7"
    )
    .parse()
    .unwrap();
    let Query(incident) = Query::<AnomalyIncidentQuery>::try_from_uri(&uri).unwrap();
    let Query(queue) = Query::<RemediationQueueQuery>::try_from_uri(&uri).unwrap();
    let incident = into_incident_filters(incident);
    let queue = into_remediation_queue_filters(queue);
    assert_eq!(incident.owner_user_id.as_deref(), Some("owner-a"));
    assert_eq!(queue.owner_user_id, incident.owner_user_id);
    assert_eq!(queue.project_id, incident.project_id);
    assert_eq!(queue.route_policy_id, incident.route_policy_id);
    assert_eq!(queue.incident_id, incident.incident_id);
    assert_eq!(queue.due_only, Some(false));
    assert_eq!(incident.due_only, Some(false));
    assert_eq!(queue.limit, Some(7));
    assert_eq!(incident.limit, Some(7));
}

#[test]
fn actor_attribution_prefers_nonempty_operator_then_user_then_management() {
    let mut headers = AxumHeaderMap::new();
    assert_eq!(management_actor_user_id(&headers), "management");
    headers.insert("x-user-id", HeaderValue::from_static("user-a"));
    assert_eq!(management_actor_user_id(&headers), "user-a");
    headers.insert("x-operator-user-id", HeaderValue::from_static("operator-a"));
    assert_eq!(management_actor_user_id(&headers), "operator-a");
    headers.insert("x-operator-user-id", HeaderValue::from_static("   "));
    assert_eq!(management_actor_user_id(&headers), "user-a");
    headers.insert(
        "x-operator-user-id",
        HeaderValue::from_bytes(&[0xff]).unwrap(),
    );
    assert_eq!(management_actor_user_id(&headers), "user-a");
    headers.insert("x-user-id", HeaderValue::from_static(""));
    assert_eq!(management_actor_user_id(&headers), "management");
}
