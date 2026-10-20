//! Reference-inspired behavior exercised through real local send/finalize seams.
#![allow(dead_code)]
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "reference_routing/cooldown.rs"]
mod cooldown;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
#[path = "reference_routing/health_policy.rs"]
mod health_policy;
#[path = "reference_routing/health_recovery.rs"]
mod health_recovery;
#[path = "reference_routing/health_store.rs"]
mod health_store;
#[path = "reference_routing/latency_queue.rs"]
mod latency_queue;
#[path = "reference_routing/latency_runtime.rs"]
mod latency_runtime;
#[path = "reference_routing/latency_sampling.rs"]
mod latency_sampling;
#[path = "reference_routing/pg.rs"]
mod pg;
#[path = "reference_routing/pg_recovery.rs"]
mod pg_recovery;
mod support;
