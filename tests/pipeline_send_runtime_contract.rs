#[path = "pipeline_send_runtime/balance_dispatch.rs"]
mod balance_dispatch;
#[path = "pipeline_send_runtime/credential_eligibility.rs"]
mod credential_eligibility;
mod support;

#[path = "pipeline_send_runtime/affinity.rs"]
mod affinity;
#[path = "pipeline_send_runtime/budget.rs"]
mod budget;
#[path = "pipeline_send_runtime/buffered.rs"]
mod buffered;
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/deadline.rs"]
mod deadline;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
#[path = "pipeline_send_runtime/html_diagnostic.rs"]
mod html_diagnostic;
#[path = "pipeline_send_runtime/model_circuit.rs"]
mod model_circuit;
#[path = "pipeline_send_runtime/rejected_channel.rs"]
mod rejected_channel;
#[path = "pipeline_send_runtime/responses_completeness.rs"]
mod responses_completeness;
#[path = "pipeline_send_runtime/safety.rs"]
mod safety;
#[path = "pipeline_send_runtime/stream_budget.rs"]
mod stream_budget;
#[path = "pipeline_send_runtime/streaming.rs"]
mod streaming;
