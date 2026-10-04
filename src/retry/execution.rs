//! Keep pre-admission, real-send start and observation as separate lifecycle seams.
use super::*;

pub async fn execute_with_retry_after_admission_observed_started<T, F, Fut, A, AFut, B, O>(
    mut f: F,
    mut admit_retry: A,
    mut begin_attempt: B,
    mut observe_attempt: O,
    policy: &RetryPolicy,
) -> Result<T, GatewayError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>>,
    A: FnMut() -> AFut,
    AFut: std::future::Future<Output = Result<(), GatewayError>>,
    B: FnMut() -> Result<(), GatewayError>,
    O: for<'a> FnMut(RetryAttemptObservation<'a>),
{
    let mut last_error: Option<GatewayError> = None;
    let mut wait_budget = WaitBudget::new(policy.max_total_wait);
    for attempt in 0..=policy.max_retries {
        if attempt > 0 {
            admit_retry().await?;
        }
        // A denied send has neither an upstream future nor a cancellation observation.
        begin_attempt()?;
        let observation = AttemptObservation::new(&mut observe_attempt, attempt > 0);
        let result = f().await;
        observation.complete(&result);
        match result {
            Ok(result) => return Ok(result),
            Err(error) => {
                if attempt < policy.max_retries && should_retry(&error, policy) {
                    let Some(delay) = wait_budget.next_delay(&error, attempt, policy) else {
                        warn!(attempt, kind = ?error.kind, "upstream retry wait budget exhausted");
                        return Err(error);
                    };
                    warn!(attempt, delay_ms = delay.as_millis() as u64, kind = ?error.kind,
                        "upstream call failed; retrying");
                    last_error = Some(error);
                    sleep(delay).await;
                } else {
                    return Err(error);
                }
            }
        }
    }
    Err(last_error.expect("retry loop exited without a result"))
}
