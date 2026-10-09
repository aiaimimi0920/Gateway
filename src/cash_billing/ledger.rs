//! Account lock + receipt write are one transaction; duplicate finalizers cannot double debit.
use super::{amount_error, sql::CashConnection, CashBalance, CashOutcome, CashReceipt};
use crate::error::GatewayError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(super) struct Account {
    pub balance: CashBalance,
    pub recorded_requests: u64,
}

pub(super) fn encode<T: Serialize>(value: &T) -> Result<String, GatewayError> {
    serde_json::to_string(value)
        .map_err(|_| GatewayError::server_error("Cannot encode cash ledger"))
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, GatewayError> {
    serde_json::from_str(value)
        .map_err(|_| GatewayError::server_error("Invalid cash ledger record"))
}

pub(super) fn decode_account(value: &str) -> Result<Account, GatewayError> {
    let account: Account = decode(value)?;
    account.balance.validate()?;
    if account.recorded_requests > 100_000
        || account.balance.pending_requests > account.recorded_requests
    {
        return Err(GatewayError::server_error("Invalid cash ledger account"));
    }
    Ok(account)
}

pub(super) fn decode_receipt(value: &str) -> Result<CashReceipt, GatewayError> {
    let receipt: CashReceipt = decode(value)?;
    receipt.validate()?;
    Ok(receipt)
}

pub(super) async fn account_id(
    db: &mut CashConnection<'_>,
    key: &str,
) -> Result<Option<String>, GatewayError> {
    db.optional(
        "SELECT account_id FROM gateway_cash_keys WHERE key_id=$1",
        &[key],
        false,
    )
    .await
}

pub(super) async fn account(
    db: &mut CashConnection<'_>,
    id: &str,
) -> Result<Account, GatewayError> {
    let payload = db
        .optional(
            "SELECT payload FROM gateway_cash_accounts WHERE id=$1",
            &[id],
            true,
        )
        .await?
        .ok_or_else(|| GatewayError::server_error("Cash account is missing"))?;
    decode_account(&payload)
}

async fn write_account(
    db: &mut CashConnection<'_>,
    id: &str,
    account: &Account,
) -> Result<(), GatewayError> {
    db.execute(
        "UPDATE gateway_cash_accounts SET payload=$2 WHERE id=$1",
        &[id, &encode(account)?],
    )
    .await
}

pub(super) async fn set_limit(
    db: &mut CashConnection<'_>,
    key: &str,
    total: i64,
) -> Result<(), GatewayError> {
    if !db.supported().await? {
        return Err(GatewayError::service_unavailable(
            "Cash quota schema is not installed on this server",
        )
        .with_code("cash_schema_required"));
    }
    if !(0..=super::MAX_MICROS).contains(&total) {
        return Err(amount_error());
    }
    if let Some(id) = account_id(db, key).await? {
        let mut account = account(db, &id).await?;
        account.balance.total_micros = total;
        return write_account(db, &id, &account).await;
    }
    let count = db
        .optional(
            "SELECT CAST(count(*) AS TEXT) FROM gateway_cash_accounts",
            &[],
            false,
        )
        .await?;
    if count
        .as_deref()
        .and_then(|count| count.parse::<u64>().ok())
        .unwrap_or(u64::MAX)
        >= 10_000
    {
        return Err(GatewayError::conflict("Cash account limit reached"));
    }
    let account = Account {
        balance: CashBalance::new(total),
        recorded_requests: 0,
    };
    db.execute(
        "INSERT INTO gateway_cash_accounts(id,payload) VALUES($1,$2)",
        &[key, &encode(&account)?],
    )
    .await?;
    db.execute(
        "INSERT INTO gateway_cash_keys(key_id,account_id) VALUES($1,$1)",
        &[key],
    )
    .await
}

pub(super) async fn reserve(
    db: &mut CashConnection<'_>,
    mut receipt: CashReceipt,
) -> Result<(), GatewayError> {
    receipt.validate()?;
    let id = account_id(db, &receipt.access_key_id)
        .await?
        .ok_or_else(|| GatewayError::quota_exceeded("Cash quota is not initialized"))?;
    let mut account = account(db, &id).await?;
    if let Some(payload) = db
        .optional(
            "SELECT payload FROM gateway_cash_requests WHERE id=$1",
            &[&receipt.request_id],
            false,
        )
        .await?
    {
        let previous = decode_receipt(&payload)?;
        if previous.access_key_id == receipt.access_key_id
            && previous.quotes == receipt.quotes
            && previous.reserved_micros == receipt.reserved_micros
            && previous.status == "reserved"
        {
            return Ok(());
        }
        return Err(GatewayError::conflict("Cash request ID cannot be reused"));
    }
    if account.recorded_requests >= 100_000 {
        return Err(GatewayError::conflict(
            "Cash account receipt capacity reached",
        ));
    }
    account.balance.reserve(receipt.reserved_micros)?;
    account.recorded_requests += 1;
    receipt.status = "reserved".into();
    db.execute(
        "INSERT INTO gateway_cash_requests(id,account_id,payload,created) VALUES($1,$2,$3,$4)",
        &[
            &receipt.request_id,
            &id,
            &encode(&receipt)?,
            &receipt.created_at,
        ],
    )
    .await?;
    write_account(db, &id, &account).await
}

pub(super) async fn finish(
    db: &mut CashConnection<'_>,
    request: &str,
    outcome: CashOutcome,
) -> Result<(), GatewayError> {
    let Some(id) = db
        .optional(
            "SELECT account_id FROM gateway_cash_requests WHERE id=$1",
            &[request],
            false,
        )
        .await?
    else {
        return Err(GatewayError::not_found("Cash reservation not found"));
    };
    let mut account = account(db, &id).await?;
    let mut receipt = decode_receipt(
        &db.optional(
            "SELECT payload FROM gateway_cash_requests WHERE id=$1",
            &[request],
            true,
        )
        .await?
        .ok_or_else(|| GatewayError::not_found("Cash reservation not found"))?,
    )?;
    if matches!(receipt.status.as_str(), "settled" | "released") {
        return Ok(());
    }
    let actual = match outcome {
        CashOutcome::NotSent => {
            receipt.status = "released".into();
            receipt.reason = Some("not_sent".into());
            Some(0)
        }
        CashOutcome::Unknown => {
            receipt.reason = Some("usage_unavailable".into());
            None
        }
        CashOutcome::Success {
            provider,
            credential,
            model,
            usage,
        } => {
            let quote = receipt
                .quotes
                .iter()
                .find(|quote| {
                    quote.provider_account_id == provider
                        && quote.credential_id == credential
                        && quote.model == model
                })
                .cloned();
            receipt.usage = usage;
            receipt.settled_quote = quote;
            match (&receipt.settled_quote, &receipt.usage) {
                (Some(quote), Some(usage)) => {
                    let amount = quote.actual_charge(usage)?;
                    receipt.status = "settled".into();
                    receipt.reason = (amount > receipt.reserved_micros)
                        .then(|| "upstream_usage_exceeded_reservation".into());
                    Some(amount)
                }
                _ => {
                    receipt.reason = Some("usage_or_tariff_unavailable".into());
                    None
                }
            }
        }
    };
    if let Some(actual) = actual {
        account.balance.settle(receipt.reserved_micros, actual)?;
        receipt.amount_micros = Some(actual);
    } else {
        // An interrupted stream or missing usage is not a zero-dollar invoice.
        receipt.status = "unresolved".into();
    }
    receipt.validate()?;
    account.balance.validate()?;
    db.execute(
        "UPDATE gateway_cash_requests SET payload=$2 WHERE id=$1",
        &[request, &encode(&receipt)?],
    )
    .await?;
    write_account(db, &id, &account).await
}

pub(super) async fn rotate(
    db: &mut CashConnection<'_>,
    old: &str,
    next: &str,
) -> Result<(), GatewayError> {
    if db.supported().await? {
        db.execute("INSERT INTO gateway_cash_keys(key_id,account_id) SELECT $2,account_id FROM gateway_cash_keys WHERE key_id=$1 ON CONFLICT(key_id) DO NOTHING", &[old, next]).await?;
    }
    Ok(())
}
