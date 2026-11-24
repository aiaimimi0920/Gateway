//! Durable intent, terminal accounting evidence, and mutation claims.
use super::{State, UpstreamId};
use crate::error::GatewayError;
use sqlx::{PgPool, SqlitePool};

mod authorization;
mod balance;
mod create;
mod dispatch;
mod ingress;
mod mutation;
mod projection;
mod route_binding;
mod stream;
use balance::{sql, Ledger};
#[cfg(test)]
use ingress::run_submit;
pub(crate) use ingress::{
    lookup_pinned, run_existing_with_stream, run_submit_with_stream, PinnedTarget, SendHooks,
};

pub(crate) const SCHEMA: &str =
    include_str!("../../../../deploy/postgres/initdb/008-gateway-sap-response-receipts.sql");
pub(crate) const MUTATION_SCHEMA: &str =
    include_str!("../../../../deploy/postgres/initdb/009-gateway-sap-response-mutations.sql");
pub(crate) const ROUTE_SCHEMA: &str =
    include_str!("../../../../deploy/postgres/initdb/010-gateway-sap-response-route-pins.sql");
pub(crate) const RECOVERY_SCHEMA: &str =
    include_str!("../../../../deploy/postgres/initdb/011-gateway-sap-response-recovery.sql");
const COLUMNS: &str = "id, owner_tag, binding_tag, provider_ref, credential_ref, upstream_id, phase, revision, input_tokens, output_tokens, settled";

enum Store<'a> {
    Local(&'a SqlitePool),
    Hosted(&'a PgPool),
}

// SQLite and PostgreSQL both accept numbered parameters; keep binds in occurrence order.
macro_rules! run {
    ($store:expr, $pool:ident => $query:expr) => {
        match $store {
            Store::Local($pool) => $query,
            Store::Hosted($pool) => $query,
        }
        .map_err(storage)
    };
}
pub(crate) mod reconciliation;
pub(crate) mod recovery;

#[derive(sqlx::FromRow)]
struct Receipt {
    id: String,
    owner_tag: String,
    binding_tag: String,
    provider_ref: String,
    credential_ref: String,
    upstream_id: Option<String>,
    phase: String,
    revision: i64,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    settled: i64,
}

#[derive(sqlx::FromRow, PartialEq, Eq)]
struct RoutePin {
    public_model: String,
    source_key_ref: String,
    platform_access_ref: Option<String>,
}

impl RoutePin {
    fn valid(&self, hosted: bool) -> bool {
        reference(&self.public_model)
            && reference(&self.source_key_ref)
            && self.platform_access_ref.is_some() == hosted
            && self.platform_access_ref.as_deref().is_none_or(reference)
    }
}

impl Receipt {
    fn terminal(&self) -> bool {
        matches!(self.phase.as_str(), "completed" | "failed")
    }
}

impl Store<'_> {
    /// Insert before a single possible upstream send. After process loss, `submitting`
    /// does not distinguish sent from not sent and must never authorize replay.
    async fn begin(
        &self,
        owner: &str,
        binding: &str,
        provider: &str,
        credential: &str,
        pin: &RoutePin,
    ) -> Result<Receipt, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx
            .insert_intent(owner, binding, provider, credential, pin)
            .await?;
        tx.commit().await?;
        Ok(row)
    }

    // SQLite and Hosted share the same storage contract.
    fn from_state(state: &crate::state::AppState) -> Option<Store<'_>> {
        if let Some(pool) = &state.pg_pool {
            Some(Store::Hosted(pool))
        } else {
            state
                .local_runtime
                .as_ref()
                .map(|local| Store::Local(&local.pool))
        }
    }
}

impl Ledger<'_> {
    async fn insert_intent(
        &mut self,
        owner: &str,
        binding: &str,
        provider: &str,
        credential: &str,
        pin: &RoutePin,
    ) -> Result<Receipt, GatewayError> {
        if !tag(owner)
            || !tag(binding)
            || !reference(provider)
            || !reference(credential)
            || !pin.valid(matches!(self, Ledger::Hosted(_)))
        {
            return Err(super::invalid());
        }
        let id = format!("resp_gw_{}", uuid::Uuid::new_v4().simple());
        let statement = format!("INSERT INTO gateway_sap_response_receipts (id, owner_tag, binding_tag, provider_ref, credential_ref, phase)
            VALUES ($1, $2, $3, $4, $5, 'submitting') RETURNING {COLUMNS}");
        let row = sql!(self, conn => sqlx::query_as::<_, Receipt>(&statement).bind(&id).bind(owner).bind(binding)
            .bind(provider).bind(credential).fetch_one(&mut **conn).await)?;
        sql!(self, conn => sqlx::query(
            "INSERT INTO gateway_sap_response_route_pins (response_id, public_model, source_key_ref, platform_access_ref)
             VALUES ($1, $2, $3, $4)")
            .bind(&id).bind(&pin.public_model).bind(&pin.source_key_ref).bind(&pin.platform_access_ref)
            .execute(&mut **conn).await.map(|_| ()))?;
        Ok(row)
    }
}

impl Store<'_> {
    /// Ownership is matched in storage. A caller still needs current authorization
    /// for the recorded provider and exact credential binding before using its ID.
    async fn read(&self, id: &str, owner: &str) -> Result<Option<Receipt>, GatewayError> {
        if !public_id(id) || !tag(owner) {
            return Ok(None);
        }
        let sql = format!(
            "SELECT {COLUMNS} FROM gateway_sap_response_receipts WHERE id = $1 AND owner_tag = $2"
        );
        run!(self, pool => sqlx::query_as::<_, Receipt>(&sql).bind(id).bind(owner).fetch_optional(*pool).await)
    }

    /// Internal receipt reads are also used for settlement. Only this lookup
    /// supplies a route identity; never derive one for a legacy receipt.
    async fn route_pin(&self, receipt: &Receipt) -> Result<Option<RoutePin>, GatewayError> {
        run!(self, pool => sqlx::query_as::<_, RoutePin>(
            "SELECT p.public_model, p.source_key_ref, p.platform_access_ref
             FROM gateway_sap_response_route_pins p JOIN gateway_sap_response_receipts r ON r.id = p.response_id
             WHERE r.id = $1 AND r.owner_tag = $2 AND r.binding_tag = $3")
            .bind(&receipt.id).bind(&receipt.owner_tag).bind(&receipt.binding_tag).fetch_optional(*pool).await)
    }

    /// The first terminal phase and the first known usage each win. A later
    /// authoritative observation can fill missing usage but never replace it.
    async fn observe(
        &self,
        receipt: &Receipt,
        upstream: &UpstreamId,
        state: State,
    ) -> Result<Option<Receipt>, GatewayError> {
        let (phase, usage) = match state {
            State::Pending => ("pending", None),
            State::Completed(usage) => ("completed", usage),
            State::Failed(usage) => ("failed", usage),
        };
        let (input, output) = match usage {
            Some(usage) => {
                let input = count(usage.prompt_tokens)?;
                let output = count(usage.completion_tokens)?;
                if input
                    .checked_add(output)
                    .and_then(|sum| u64::try_from(sum).ok())
                    != Some(usage.total_tokens)
                {
                    return Err(super::failure("sap_aicore_response_failed"));
                }
                (Some(input), Some(output))
            }
            None => (None, None),
        };
        let sql = format!("UPDATE gateway_sap_response_receipts
            SET upstream_id = $1, phase = $2, input_tokens = $3, output_tokens = $4, revision = revision + 1
            WHERE id = $5 AND owner_tag = $6 AND binding_tag = $7 AND revision = $8
              AND (phase = 'submitting' OR (upstream_id = $1 AND (
                  (phase = 'pending' AND $2 <> 'pending')
                  OR (phase = $2 AND phase IN ('completed', 'failed') AND settled = 0
                      AND input_tokens IS NULL AND $3 IS NOT NULL))))
            RETURNING {COLUMNS}");
        run!(self, pool => sqlx::query_as::<_, Receipt>(&sql).bind(&upstream.0).bind(phase).bind(input).bind(output)
            .bind(&receipt.id).bind(&receipt.owner_tag).bind(&receipt.binding_tag).bind(receipt.revision)
            .fetch_optional(*pool).await)
    }
}

fn tag(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

fn public_id(s: &str) -> bool {
    s.strip_prefix("resp_gw_").is_some_and(|v| {
        v.len() == 32
            && v.bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    })
}

fn reference(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && s.trim() == s && !s.chars().any(char::is_control)
}

fn count(value: u64) -> Result<i64, GatewayError> {
    i64::try_from(value).map_err(|_| super::failure("sap_aicore_response_failed"))
}

fn storage(error: sqlx::Error) -> GatewayError {
    let code = error.as_database_error().and_then(|db| db.code());
    tracing::warn!(sqlstate = ?code, kind = ?std::mem::discriminant(&error), "SAP Responses receipt operation failed");
    GatewayError::service_unavailable("SAP Responses receipt storage unavailable.")
        .with_code("sap_aicore_response_receipt_unavailable")
}

#[cfg(test)]
mod tests;
