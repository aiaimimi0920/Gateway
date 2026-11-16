//! One generation submission, then read only its full identity set until terminal or deadline.
use super::{direct_state, transport};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayRequest,
        suno::{self, SunoClip},
    },
    upstream::{
        client::UpstreamClient,
        suno_runtime_helpers::{
            build_suno_challenge_check_plan, build_suno_feed_poll_plan, build_suno_generate_plan,
            PreparedSunoExecutionContext,
        },
    },
};

pub(super) async fn run(
    client: &UpstreamClient,
    context: &PreparedSunoExecutionContext,
    req: &CanonicalRelayRequest,
    model: &str,
    deadline: tokio::time::Instant,
) -> Result<(Vec<SunoClip>, bool), GatewayError> {
    // Validate/serialize the generation before even the challenge request.
    let generation =
        build_suno_generate_plan(&context.base_url, req, model, context.user_tier.as_deref())?;
    let mut total = 0;
    let probe = transport::send(
        client,
        &build_suno_challenge_check_plan(&context.base_url, req.endpoint_kind),
        context,
        deadline,
        &mut total,
    )
    .await?;
    if suno::challenge_required(&probe)? && context.execution_plan.missing_challenge_token {
        return Err(crate::upstream::suno_response_helpers::suno_challenge_required_error());
    }
    let body = transport::send(client, &generation, context, deadline, &mut total).await?;
    let mut clips = direct_state::snapshot(&body, None)?;
    let ids: Vec<_> = clips.iter().map(|c| c.id.clone()).collect();
    loop {
        let ready =
            clips.len() == ids.len() && suno::clips_ready_for_endpoint(req.endpoint_kind, &clips);
        if ready || !context.execution_plan.wait_completion {
            return Ok((clips, ready));
        }
        tokio::time::sleep(context.execution_plan.poll_interval).await;
        let body = transport::send(
            client,
            &build_suno_feed_poll_plan(&context.base_url, req.endpoint_kind, &ids),
            context,
            deadline,
            &mut total,
        )
        .await?;
        // An empty/subset feed may be eventually consistent, but can never prove full completion.
        clips = direct_state::snapshot(&body, Some(&ids))?;
    }
}
