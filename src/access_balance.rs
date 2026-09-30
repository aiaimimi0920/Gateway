//! One balance operation interface for SQLite and PostgreSQL. Redis is not a ledger.
use crate::db::{AccessBalanceDecision, AccessKeyBalanceAdjustInput, GatewayAccessKeyBalanceView};
use crate::error::GatewayError;
use crate::local_runtime::LocalRuntime;
use crate::state::AppState;

pub(crate) mod policy;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mutation {
    Adjust,
    Reserve,
    Finalize,
}

pub(crate) type Change<T> = (GatewayAccessKeyBalanceView, T, i64);

#[derive(Clone, Copy)]
pub enum AccessBalanceStore<'a> {
    Sqlite(&'a LocalRuntime),
    Postgres(&'a sqlx::PgPool),
}

impl<'a> AccessBalanceStore<'a> {
    pub fn from_state(state: &'a AppState) -> Result<Self, GatewayError> {
        if let Some(local) = &state.local_runtime {
            Ok(Self::Sqlite(local))
        } else {
            state.pg_pool.as_ref().map(Self::Postgres).ok_or_else(|| {
                GatewayError::service_unavailable("Access key storage is not configured")
            })
        }
    }

    pub async fn get(&self, id: &str) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
        match self {
            Self::Sqlite(local) => local.access_balance(id).await,
            Self::Postgres(pool) => crate::db::access::balance_store::load(pool, id).await,
        }
    }

    pub async fn evaluate(
        &self,
        id: &str,
        estimate: u64,
    ) -> Result<AccessBalanceDecision, GatewayError> {
        Ok(policy::evaluate(
            &self.get(id).await?.ok_or_else(uninitialized)?,
            estimate,
        ))
    }

    pub async fn adjust(
        &self,
        id: &str,
        input: AccessKeyBalanceAdjustInput,
    ) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
        self.mutate(id, Mutation::Adjust, |current| {
            let balance = policy::adjust(id, current, input)?;
            Ok((balance.clone(), balance, 0))
        })
        .await
    }

    pub async fn reserve(
        &self,
        id: &str,
        estimate: u64,
    ) -> Result<AccessBalanceDecision, GatewayError> {
        self.mutate(id, Mutation::Reserve, |current| {
            let mut balance = current.ok_or_else(uninitialized)?;
            let decision = policy::reserve(&mut balance, estimate)?;
            let pending = i64::from(decision.allowed && decision.pre_deduct_amount > 0);
            Ok((balance, decision, pending))
        })
        .await
    }

    pub async fn refund(&self, id: &str, reserved: u64) -> Result<(), GatewayError> {
        if reserved == 0 {
            return Ok(());
        }
        self.mutate(id, Mutation::Finalize, |current| {
            let mut balance = current.ok_or_else(uninitialized)?;
            policy::refund(&mut balance, reserved, true)?;
            Ok((balance, (), -1))
        })
        .await
    }

    pub async fn settle(&self, id: &str, reserved: u64, actual: u64) -> Result<(), GatewayError> {
        if reserved == 0 {
            return Ok(());
        }
        self.mutate(id, Mutation::Finalize, |current| {
            let mut balance = current.ok_or_else(uninitialized)?;
            policy::settle(&mut balance, reserved, actual)?;
            Ok((balance, (), -1))
        })
        .await
    }

    async fn mutate<T, F>(
        &self,
        id: &str,
        operation: Mutation,
        change: F,
    ) -> Result<T, GatewayError>
    where
        F: FnOnce(Option<GatewayAccessKeyBalanceView>) -> Result<Change<T>, GatewayError>,
    {
        match self {
            Self::Sqlite(local) => local.mutate_access_balance(id, operation, change).await,
            Self::Postgres(pool) => {
                crate::db::access::balance_store::mutate(pool, id, operation, change).await
            }
        }
    }
}

fn uninitialized() -> GatewayError {
    GatewayError::quota_exceeded("Access key balance is not initialized")
        .with_code("balance_not_initialized")
}
