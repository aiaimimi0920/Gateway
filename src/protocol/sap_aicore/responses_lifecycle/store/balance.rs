//! Receipt-bound accounting: the same transaction commits the debit or settlement
//! and its idempotency marker. No process lease owns these holds.
use super::{storage, Receipt, RoutePin, Store, COLUMNS};
use crate::{
    access_balance::{policy, Mutation},
    db::{access::balance_store as hosted, GatewayAccessKeyBalanceView},
    error::GatewayError,
    local_runtime::access_balances as local,
};
use sqlx::{Postgres, Sqlite, Transaction};

pub(super) enum Ledger<'a> {
    Local(Transaction<'a, Sqlite>),
    Hosted(Transaction<'a, Postgres>),
}

macro_rules! sql {
    ($tx:expr, $c:ident => $query:expr) => {
        match $tx {
            Ledger::Local($c) => $query,
            Ledger::Hosted($c) => $query,
        }
        .map_err(storage)
    };
}
pub(super) use sql;

#[derive(sqlx::FromRow)]
struct Hold {
    key_ref: String,
    account_ref: String,
    mode: String,
    reserved: i64,
    open: i64,
}

impl Store<'_> {
    pub(super) async fn ledger(&self) -> Result<Ledger<'_>, GatewayError> {
        match self {
            Self::Local(pool) => pool.begin_with("BEGIN IMMEDIATE").await.map(Ledger::Local),
            Self::Hosted(pool) => pool.begin().await.map(Ledger::Hosted),
        }
        .map_err(storage)
    }

    /// `true` authorizes one possible send. A duplicate only proves a debit
    /// committed earlier; it must never be used to retry an uncertain dispatch.
    pub(super) async fn reserve_balance(
        &self,
        receipt: &Receipt,
        key: &str,
        estimate: u64,
    ) -> Result<bool, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(receipt).await?;
        if let Some(hold) = tx.hold(&row.id).await? {
            return if hold.key_ref == key {
                Ok(false)
            } else {
                Err(conflict())
            };
        }
        if row.phase != "submitting" || row.revision != receipt.revision {
            return Err(conflict());
        }
        let source: Option<String> = sql!(&mut tx, conn => sqlx::query_scalar(
            "SELECT source_key_ref FROM gateway_sap_response_route_pins WHERE response_id = $1")
            .bind(&row.id).fetch_optional(&mut **conn).await)?;
        if source.as_deref() != Some(key) {
            return Err(conflict());
        }
        tx.debit(&row, key, estimate).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub(super) async fn begin_reserved(
        &self,
        owner: &str,
        binding: &str,
        provider: &str,
        credential: &str,
        pin: &RoutePin,
        estimate: u64,
    ) -> Result<Receipt, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx
            .insert_intent(owner, binding, provider, credential, pin)
            .await?;
        tx.debit(&row, &pin.source_key_ref, estimate).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Only before execute is entered. The closed hold prevents a second debit
    /// without asserting an SAP terminal status or inventing an upstream ID.
    pub(super) async fn refund_unsent(&self, receipt: &Receipt) -> Result<(), GatewayError> {
        // Commit proof separately so a failed refund remains recoverable after a crash.
        // A missing migration cannot invalidate the live owner's direct unsent knowledge.
        if let Err(error) = self.record_unsent(receipt).await {
            tracing::warn!(code = ?error.code, "SAP unsent proof unavailable; trying live-owner refund only");
        }
        self.refund_checked(receipt, false).await.map(|_| ())
    }

    pub(super) async fn refund_proven_unsent(
        &self,
        receipt: &Receipt,
    ) -> Result<bool, GatewayError> {
        self.refund_checked(receipt, true).await
    }

    async fn refund_checked(
        &self,
        receipt: &Receipt,
        require_proof: bool,
    ) -> Result<bool, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(receipt).await?;
        if row.phase != "submitting"
            || row.upstream_id.is_some()
            || row.revision != receipt.revision
        {
            return Err(conflict());
        }
        if require_proof && !tx.proven_unsent(&row).await? {
            return Err(conflict());
        }
        let hold = tx.hold(&row.id).await?.ok_or_else(conflict)?;
        if hold.open == 0 {
            return Ok(false);
        }
        if hold.reserved > 0 {
            let (account, mut balance) = tx.balance(&hold.key_ref, Mutation::Finalize).await?;
            if account != hold.account_ref || balance.balance_mode != hold.mode {
                return Err(conflict());
            }
            policy::refund(
                &mut balance,
                u64::try_from(hold.reserved).map_err(|_| conflict())?,
                true,
            )?;
            tx.write(&account, &balance).await?;
        }
        tx.close_hold(&row.id).await?;
        tx.cleanup_local().await?;
        tx.commit().await?;
        Ok(true)
    }
}

impl Ledger<'_> {
    async fn debit(&mut self, row: &Receipt, key: &str, estimate: u64) -> Result<(), GatewayError> {
        let (account, mut balance) = self.balance(key, Mutation::Reserve).await?;
        if balance.balance_mode == "cash_prepaid" {
            return Err(GatewayError::bad_request(
                "Stored SAP Responses cash tariff is not yet supported",
            )
            .with_code("sap_aicore_response_cash_unsupported"));
        }
        let decision = policy::reserve(&mut balance, estimate)?;
        if !decision.allowed {
            return Err(GatewayError::quota_exceeded(
                "Access key balance rejected stored SAP Responses",
            )
            .with_code("sap_aicore_response_balance_denied"));
        }
        let reserved = i64::try_from(decision.pre_deduct_amount).map_err(|_| conflict())?;
        sql!(self, conn => sqlx::query("INSERT INTO gateway_sap_response_balance_holds
            (response_id,key_ref,account_ref,mode,reserved) VALUES ($1,$2,$3,$4,$5)")
            .bind(&row.id).bind(key).bind(&account).bind(&balance.balance_mode)
            .bind(reserved).execute(&mut **conn).await.map(|_| ()))?;
        self.write(&account, &balance).await
    }

    async fn close_hold(&mut self, id: &str) -> Result<(), GatewayError> {
        let changed = sql!(self, conn => sqlx::query(
            "UPDATE gateway_sap_response_balance_holds SET open = 0 WHERE response_id = $1 AND open = 1")
            .bind(id).execute(&mut **conn).await.map(|result| result.rows_affected()))?;
        if changed != 1 {
            return Err(conflict());
        }
        Ok(())
    }

    async fn cleanup_local(&mut self) -> Result<(), GatewayError> {
        if let Ledger::Local(conn) = self {
            local::cleanup(conn).await?;
        }
        Ok(())
    }
}

impl Store<'_> {
    /// A terminal observation with unknown charge is not fabricated as zero.
    /// Leave the debit open for reconciliation (including unknown failed work).
    pub(super) async fn settle_balance(
        &self,
        receipt: &Receipt,
    ) -> Result<Option<Receipt>, GatewayError> {
        self.settle_terminal(receipt, false).await
    }

    pub(super) async fn settle_recovered_balance(
        &self,
        receipt: &Receipt,
    ) -> Result<Option<Receipt>, GatewayError> {
        self.settle_terminal(receipt, true).await
    }

    async fn settle_terminal(
        &self,
        receipt: &Receipt,
        recovery_only: bool,
    ) -> Result<Option<Receipt>, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(receipt).await?;
        if !row.terminal() || row.settled != 0 {
            return Ok(None);
        }
        let hold = tx
            .hold(&row.id)
            .await?
            .filter(|hold| hold.open == 1)
            .ok_or_else(conflict)?;
        let actual = match (row.input_tokens, row.output_tokens) {
            (Some(input), Some(output)) => Some(
                u64::try_from(input).map_err(|_| conflict())?
                    + u64::try_from(output).map_err(|_| conflict())?,
            ),
            _ => None,
        };
        // A background scan is only a hint. Recheck its exact no-usage exception
        // under the receipt lock, without widening recovery to zero/other holds.
        if recovery_only
            && actual.is_none()
            && !(row.phase == "completed"
                && row.input_tokens.is_none()
                && row.output_tokens.is_none()
                && hold.mode == "message_prepaid"
                && hold.reserved == 1)
        {
            return Ok(None);
        }
        if hold.reserved > 0
            && actual.is_none()
            && (hold.mode == "token_prepaid" || row.phase == "failed")
        {
            return Ok(None);
        }
        if hold.reserved > 0 {
            let (account, mut balance) = tx.balance(&hold.key_ref, Mutation::Finalize).await?;
            if account != hold.account_ref || balance.balance_mode != hold.mode {
                return Err(conflict());
            }
            let reserved = u64::try_from(hold.reserved).map_err(|_| conflict())?;
            if row.phase == "failed" && actual == Some(0) {
                policy::refund(&mut balance, reserved, true)?;
            } else if hold.mode == "token_prepaid" {
                policy::settle(&mut balance, reserved, actual.ok_or_else(conflict)?)?;
            }
            tx.write(&account, &balance).await?;
        }
        tx.close_hold(&row.id).await?;
        let sql = format!("UPDATE gateway_sap_response_receipts SET settled = 1, revision = revision + 1 WHERE id = $1 RETURNING {COLUMNS}");
        let settled = sql!(&mut tx, conn => sqlx::query_as::<_, Receipt>(&sql)
            .bind(&row.id).fetch_one(&mut **conn).await)?;
        tx.cleanup_local().await?;
        tx.commit().await?;
        Ok(Some(settled))
    }
}

impl Ledger<'_> {
    pub(super) async fn lock_receipt(
        &mut self,
        receipt: &Receipt,
    ) -> Result<Receipt, GatewayError> {
        let sql = format!("SELECT {COLUMNS} FROM gateway_sap_response_receipts WHERE id = $1 AND owner_tag = $2 AND binding_tag = $3{}",
            if matches!(self, Self::Hosted(_)) { " FOR UPDATE" } else { "" });
        sql!(self, conn => sqlx::query_as::<_, Receipt>(&sql)
            .bind(&receipt.id).bind(&receipt.owner_tag).bind(&receipt.binding_tag)
            .fetch_optional(&mut **conn).await)?
        .ok_or_else(conflict)
    }

    async fn hold(&mut self, id: &str) -> Result<Option<Hold>, GatewayError> {
        sql!(self, conn => sqlx::query_as::<_, Hold>("SELECT key_ref, account_ref, mode, reserved, open
                FROM gateway_sap_response_balance_holds WHERE response_id = $1")
            .bind(id).fetch_optional(&mut **conn).await)
    }

    async fn balance(
        &mut self,
        key: &str,
        operation: Mutation,
    ) -> Result<(String, GatewayAccessKeyBalanceView), GatewayError> {
        match self {
            Self::Local(tx) => local::lock(tx, key, operation).await,
            Self::Hosted(tx) => Ok((
                key.into(),
                hosted::lock(tx, key, operation)
                    .await?
                    .ok_or_else(conflict)?,
            )),
        }
    }

    async fn write(
        &mut self,
        account: &str,
        balance: &GatewayAccessKeyBalanceView,
    ) -> Result<(), GatewayError> {
        match self {
            Self::Local(tx) => local::write_balance(tx, account, balance).await,
            Self::Hosted(tx) => hosted::write(tx, account, balance).await,
        }
    }

    pub(super) async fn commit(self) -> Result<(), GatewayError> {
        match self {
            Self::Local(tx) => tx.commit().await,
            Self::Hosted(tx) => tx.commit().await,
        }
        .map_err(storage)
    }
}

fn conflict() -> GatewayError {
    GatewayError::conflict("Stored SAP Responses balance receipt does not match")
        .with_code("sap_aicore_response_balance_conflict")
}
