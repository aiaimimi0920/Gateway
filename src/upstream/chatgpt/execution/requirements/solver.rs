use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

use super::super::super::PROVIDER;
use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse::{self as surface, ChatGptWebBootstrap};

static SOLVER_SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub(super) async fn legacy(
    bootstrap: &ChatGptWebBootstrap,
    user_agent: &str,
) -> Result<String, GatewayError> {
    run_with(SOLVER_SLOTS.clone(), || {
        let bootstrap = bootstrap.clone();
        let user_agent = user_agent.to_string();
        move || {
            Ok(surface::build_legacy_requirements_token(
                &bootstrap,
                &user_agent,
            ))
        }
    })
    .await
}

pub(super) async fn proof(
    bootstrap: &ChatGptWebBootstrap,
    user_agent: &str,
    seed: &str,
    difficulty: &str,
) -> Result<String, GatewayError> {
    run_with(SOLVER_SLOTS.clone(), || {
        let bootstrap = bootstrap.clone();
        let user_agent = user_agent.to_string();
        let seed = seed.to_string();
        let difficulty = difficulty.to_string();
        move || surface::build_proof_token(&bootstrap, &user_agent, &seed, &difficulty)
    })
    .await
}

pub(super) async fn turnstile(
    dx: &str,
    legacy_token: &str,
) -> Result<Option<String>, GatewayError> {
    run_with(SOLVER_SLOTS.clone(), || {
        let dx = dx.to_string();
        let legacy_token = legacy_token.to_string();
        move || {
            Ok(surface::solve_turnstile_token(&dx, "")
                .or_else(|| surface::solve_turnstile_token(&dx, &legacy_token)))
        }
    })
    .await
}

async fn run_with<T, Job>(
    slots: Arc<Semaphore>,
    prepare: impl FnOnce() -> Job,
) -> Result<T, GatewayError>
where
    T: Send + 'static,
    Job: FnOnce() -> Result<T, GatewayError> + Send + 'static,
{
    // Reject before copying inputs; admitted queued and running jobs share this bound.
    let permit = slots.try_acquire_owned().map_err(|_| {
        GatewayError::service_unavailable("ChatGPT Web reverse proof solver is busy.")
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_solver_busy")
    })?;
    let job = prepare();
    let mut task = AbortOnDrop(tokio::task::spawn_blocking(move || {
        // Running work retains capacity and owned inputs even after caller cancellation.
        let _permit = permit;
        job()
    }));
    (&mut task.0).await.map_err(|_| {
        GatewayError::server_error("ChatGPT Web reverse proof solver worker failed.")
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_solver_worker_failed")
    })?
}

struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        // Queued blocking jobs can be cancelled; already-running solvers finish normally.
        self.0.abort();
    }
}

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;
