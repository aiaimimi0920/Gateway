//! Small typed SQL adapter: shared ledger SQL, native transactions and row locks.
use crate::{access_balance::AccessBalanceStore, error::GatewayError};
use sqlx::{PgConnection, Postgres, Sqlite, SqliteConnection, Transaction};

pub(crate) enum CashConnection<'a> {
    Sqlite(&'a mut SqliteConnection),
    Postgres(&'a mut PgConnection),
}

fn sql_error(error: sqlx::Error) -> GatewayError {
    tracing::error!(error = %error, "Cash ledger storage operation failed");
    GatewayError::service_unavailable("Cash ledger storage operation failed")
        .with_code("cash_storage_error")
}

impl CashConnection<'_> {
    pub(crate) async fn pairs(&mut self, sql: &str) -> Result<Vec<(String, String)>, GatewayError> {
        match self {
            Self::Sqlite(connection) => sqlx::query_as(sql).fetch_all(&mut **connection).await,
            Self::Postgres(connection) => sqlx::query_as(sql).fetch_all(&mut **connection).await,
        }
        .map_err(sql_error)
    }

    pub(crate) async fn rows(
        &mut self,
        sql: &str,
        values: &[&str],
    ) -> Result<Vec<String>, GatewayError> {
        match self {
            Self::Sqlite(connection) => {
                let mut query = sqlx::query_scalar::<_, String>(sql);
                for value in values {
                    query = query.bind(*value);
                }
                query.fetch_all(&mut **connection).await.map_err(sql_error)
            }
            Self::Postgres(connection) => {
                let mut query = sqlx::query_scalar::<_, String>(sql);
                for value in values {
                    query = query.bind(*value);
                }
                query.fetch_all(&mut **connection).await.map_err(sql_error)
            }
        }
    }

    pub(crate) async fn optional(
        &mut self,
        sql: &str,
        values: &[&str],
        lock: bool,
    ) -> Result<Option<String>, GatewayError> {
        let sql = if lock && matches!(self, Self::Postgres(_)) {
            format!("{sql} FOR UPDATE")
        } else {
            sql.into()
        };
        Ok(self.rows(&sql, values).await?.into_iter().next())
    }

    pub(crate) async fn execute(&mut self, sql: &str, values: &[&str]) -> Result<(), GatewayError> {
        match self {
            Self::Sqlite(connection) => {
                let mut query = sqlx::query(sql);
                for value in values {
                    query = query.bind(*value);
                }
                query.execute(&mut **connection).await.map_err(sql_error)?;
            }
            Self::Postgres(connection) => {
                let mut query = sqlx::query(sql);
                for value in values {
                    query = query.bind(*value);
                }
                query.execute(&mut **connection).await.map_err(sql_error)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn supported(&mut self) -> Result<bool, GatewayError> {
        let sql = match self {
            Self::Sqlite(_) => "SELECT 'ready' WHERE (SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('gateway_cash_accounts','gateway_cash_keys','gateway_cash_requests'))=3",
            Self::Postgres(_) => "SELECT 'ready' WHERE to_regclass('gateway_cash_accounts') IS NOT NULL AND to_regclass('gateway_cash_keys') IS NOT NULL AND to_regclass('gateway_cash_requests') IS NOT NULL",
        };
        Ok(self.optional(sql, &[], false).await?.is_some())
    }
}

pub(crate) enum CashTransaction {
    Sqlite(Transaction<'static, Sqlite>),
    Postgres(Transaction<'static, Postgres>),
}

impl CashTransaction {
    pub(crate) async fn begin(store: AccessBalanceStore<'_>) -> Result<Self, GatewayError> {
        match store {
            AccessBalanceStore::Sqlite(local) => local
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map(Self::Sqlite),
            AccessBalanceStore::Postgres(pool) => pool.begin().await.map(Self::Postgres),
        }
        .map_err(sql_error)
    }

    pub(crate) fn connection(&mut self) -> CashConnection<'_> {
        match self {
            Self::Sqlite(tx) => CashConnection::Sqlite(tx),
            Self::Postgres(tx) => CashConnection::Postgres(tx),
        }
    }

    pub(crate) async fn commit(self) -> Result<(), GatewayError> {
        match self {
            Self::Sqlite(tx) => tx.commit().await,
            Self::Postgres(tx) => tx.commit().await,
        }
        .map_err(sql_error)
    }
}
