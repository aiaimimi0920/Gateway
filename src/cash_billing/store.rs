//! Public storage facade and transaction-local hooks used by Key edits/rotation.
use super::{
    ledger,
    sql::{CashConnection, CashTransaction},
    CashBalance, CashOutcome, CashReceipt,
};
use crate::{
    access_balance::AccessBalanceStore, db::GatewayAccessKeyBalanceView, error::GatewayError,
};

pub(crate) async fn set_limit(
    db: &mut CashConnection<'_>,
    id: &str,
    total: i64,
) -> Result<(), GatewayError> {
    ledger::set_limit(db, id, total).await
}

pub(crate) async fn rotate(
    db: &mut CashConnection<'_>,
    old: &str,
    next: &str,
) -> Result<(), GatewayError> {
    ledger::rotate(db, old, next).await
}

impl AccessBalanceStore<'_> {
    pub async fn cash_supported(self) -> Result<bool, GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        tx.connection().supported().await
    }

    pub async fn cash_balance(self, key: &str) -> Result<Option<CashBalance>, GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        let mut db = tx.connection();
        if !db.supported().await? {
            return Ok(None);
        }
        let Some(id) = ledger::account_id(&mut db, key).await? else {
            return Ok(None);
        };
        Ok(Some(ledger::account(&mut db, &id).await?.balance))
    }

    pub async fn attach_cash(
        self,
        balances: &mut [GatewayAccessKeyBalanceView],
    ) -> Result<(), GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        let mut db = tx.connection();
        if !db.supported().await? {
            return Ok(());
        }
        // Old bindings remain for historical receipts; catalog reads only extant keys.
        let sql = match db {
            CashConnection::Sqlite(_) => "SELECT k.key_id,a.payload FROM gateway_cash_keys k JOIN gateway_cash_accounts a ON a.id=k.account_id JOIN local_access_keys live ON live.id=k.key_id LIMIT 100001",
            CashConnection::Postgres(_) => "SELECT k.key_id,a.payload FROM gateway_cash_keys k JOIN gateway_cash_accounts a ON a.id=k.account_id JOIN gateway_access_keys live ON live.id=k.key_id LIMIT 100001",
        };
        let rows = db.pairs(sql).await?;
        if rows.len() > 100_000 {
            return Err(GatewayError::service_unavailable(
                "Cash key index capacity exceeded",
            ));
        }
        let lookup = rows
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        for balance in balances {
            if let Some(payload) = lookup.get(&balance.access_key_id) {
                balance.cash = Some(ledger::decode_account(payload)?.balance);
            }
        }
        Ok(())
    }

    pub async fn cash_ledger(self, key: &str) -> Result<Vec<CashReceipt>, GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        let mut db = tx.connection();
        if !db.supported().await? {
            return Ok(Vec::new());
        }
        db.rows("SELECT r.payload FROM gateway_cash_requests r JOIN gateway_cash_keys k ON k.account_id=r.account_id WHERE k.key_id=$1 ORDER BY r.created DESC,r.id DESC LIMIT 100", &[key])
            .await?.iter().map(|value| ledger::decode_receipt(value)).collect()
    }

    pub(crate) async fn reserve_cash(self, receipt: CashReceipt) -> Result<(), GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        let mut db = tx.connection();
        require_cash_key(&mut db, &receipt.access_key_id).await?;
        ledger::reserve(&mut db, receipt).await?;
        tx.commit().await
    }

    pub(crate) async fn finish_cash(
        self,
        id: &str,
        outcome: CashOutcome,
    ) -> Result<(), GatewayError> {
        let mut tx = CashTransaction::begin(self).await?;
        ledger::finish(&mut tx.connection(), id, outcome).await?;
        tx.commit().await
    }
}

async fn require_cash_key(db: &mut CashConnection<'_>, key: &str) -> Result<(), GatewayError> {
    let active = match db {
        CashConnection::Sqlite(_) => {
            let payload = db.optional("SELECT payload FROM local_access_keys WHERE id=$1", &[key], true).await?
                .ok_or_else(|| GatewayError::unauthorized("Access key not found"))?;
            let key: crate::db::GatewayAccessKeyView = ledger::decode(&payload)?;
            let expiry = key.expires_at.as_deref().map(|value| time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)).transpose()
                .map_err(|_| GatewayError::unauthorized("Access key expiry is invalid"))?;
            key.status == "active" && key.key_kind == "normal" && expiry.is_none_or(|expiry| expiry > time::OffsetDateTime::now_utc())
        }
        CashConnection::Postgres(_) => db.optional("SELECT id FROM gateway_access_keys WHERE id=$1 AND status='active' AND key_kind='normal' AND (expires_at IS NULL OR expires_at>now())", &[key], true).await?.is_some(),
    };
    if !active {
        return Err(GatewayError::unauthorized(
            "Cash key is inactive or expired",
        ));
    }
    let allowed = match db {
        CashConnection::Sqlite(_) => {
            let payload = db.optional("SELECT a.payload FROM local_balance_accounts a JOIN local_balance_keys k ON k.account_id=a.id WHERE k.key_id=$1", &[key], true).await?
                .ok_or_else(|| GatewayError::quota_exceeded("Cash balance is missing"))?;
            let balance: GatewayAccessKeyBalanceView = ledger::decode(&payload)?;
            balance.balance_mode == "cash_prepaid" && crate::access_balance::policy::evaluate(&balance, 1).allowed
        }
        CashConnection::Postgres(_) => db.optional("SELECT balance_mode FROM gateway_access_key_balances WHERE access_key_id=$1 AND status='active' AND (period_starts_at IS NULL OR period_starts_at<=now()) AND (period_ends_at IS NULL OR period_ends_at>now())", &[key], true).await?.as_deref() == Some("cash_prepaid"),
    };
    if !allowed {
        return Err(GatewayError::conflict(
            "Cash quota changed before admission; retry the request",
        )
        .with_code("cash_quota_changed"));
    }
    Ok(())
}
