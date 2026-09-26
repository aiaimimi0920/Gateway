use super::nodes::list_browser_executor_nodes;
use super::normalization::normalize_string;
use super::slots::list_browser_capability_slots;
use super::timestamps::now_rfc3339;
use super::{
    BrowserCapabilitySlotFilters, BrowserCapabilitySlotStatus, BrowserCapabilitySlotView,
    BrowserExecutorHealthFilters, BrowserExecutorHealthView, BrowserExecutorNodeHealthView,
    BrowserExecutorProviderHealthView,
};
use crate::error::GatewayError;
use crate::state::AppState;
pub async fn get_browser_executor_health(
    state: &AppState,
    filters: BrowserExecutorHealthFilters,
) -> Result<BrowserExecutorHealthView, GatewayError> {
    let nodes = list_browser_executor_nodes(state).await?;
    let slots = list_browser_capability_slots(
        state,
        BrowserCapabilitySlotFilters {
            provider_account_id: filters.provider_account_id.clone(),
            node_id: filters.node_id.clone(),
            ..BrowserCapabilitySlotFilters::default()
        },
    )
    .await?;

    let filter_node_id = normalize_string(filters.node_id);
    let filtered_nodes = nodes
        .into_iter()
        .filter(|node| {
            filter_node_id
                .as_deref()
                .map(|value| node.node_id == value)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let mut node_health = std::collections::HashMap::<String, BrowserExecutorNodeHealthView>::new();
    let mut provider_health =
        std::collections::HashMap::<String, BrowserExecutorProviderHealthView>::new();

    for slot in &slots {
        let challenge_open = is_challenge_open(slot);
        let degraded = challenge_open
            || slot
                .degradation_reasons
                .as_ref()
                .map(|reasons| !reasons.is_empty())
                .unwrap_or(false)
            || matches!(
                slot.status,
                BrowserCapabilitySlotStatus::Cooling | BrowserCapabilitySlotStatus::Dead
            );

        let node_entry = node_health.entry(slot.node_id.clone()).or_insert_with(|| {
            BrowserExecutorNodeHealthView {
                node_id: slot.node_id.clone(),
                total_slots: 0,
                warm_slots: 0,
                busy_slots: 0,
                cooling_slots: 0,
                degraded_slots: 0,
                challenge_open_slots: 0,
                last_error_summary: None,
                updated_at: slot.updated_at.clone(),
            }
        });
        update_health_entry(
            node_entry,
            slot,
            challenge_open,
            degraded,
            slot.updated_at.clone(),
        );

        let provider_entry = provider_health
            .entry(slot.provider_account_id.clone())
            .or_insert_with(|| BrowserExecutorProviderHealthView {
                provider_account_id: slot.provider_account_id.clone(),
                total_slots: 0,
                warm_slots: 0,
                busy_slots: 0,
                cooling_slots: 0,
                degraded_slots: 0,
                challenge_open_slots: 0,
                last_error_summary: None,
                updated_at: slot.updated_at.clone(),
            });
        update_health_entry(
            provider_entry,
            slot,
            challenge_open,
            degraded,
            slot.updated_at.clone(),
        );
    }

    let mut node_views = filtered_nodes
        .into_iter()
        .map(|node| {
            node_health
                .remove(&node.node_id)
                .unwrap_or(BrowserExecutorNodeHealthView {
                    node_id: node.node_id,
                    total_slots: 0,
                    warm_slots: 0,
                    busy_slots: 0,
                    cooling_slots: 0,
                    degraded_slots: 0,
                    challenge_open_slots: 0,
                    last_error_summary: None,
                    updated_at: node.last_heartbeat_at,
                })
        })
        .collect::<Vec<_>>();
    node_views.sort_by(|left, right| left.node_id.cmp(&right.node_id));

    let mut provider_views = provider_health.into_values().collect::<Vec<_>>();
    provider_views.sort_by(|left, right| left.provider_account_id.cmp(&right.provider_account_id));

    Ok(BrowserExecutorHealthView {
        generated_at: now_rfc3339(),
        total_nodes: node_views.len(),
        total_slots: slots.len(),
        total_warm_slots: slots
            .iter()
            .filter(|slot| {
                matches!(
                    slot.status,
                    BrowserCapabilitySlotStatus::Warm | BrowserCapabilitySlotStatus::Hot
                )
            })
            .count(),
        total_busy_slots: slots
            .iter()
            .filter(|slot| slot.status == BrowserCapabilitySlotStatus::Busy)
            .count(),
        total_cooling_slots: slots
            .iter()
            .filter(|slot| slot.status == BrowserCapabilitySlotStatus::Cooling)
            .count(),
        total_degraded_slots: slots
            .iter()
            .filter(|slot| {
                slot.degradation_reasons
                    .as_ref()
                    .map(|reasons| !reasons.is_empty())
                    .unwrap_or(false)
                    || matches!(
                        slot.status,
                        BrowserCapabilitySlotStatus::Cooling | BrowserCapabilitySlotStatus::Dead
                    )
                    || is_challenge_open(slot)
            })
            .count(),
        total_challenge_open_slots: slots.iter().filter(|slot| is_challenge_open(slot)).count(),
        nodes: node_views,
        providers: provider_views,
    })
}

trait BrowserExecutorHealthEntry {
    fn total_slots_mut(&mut self) -> &mut usize;
    fn warm_slots_mut(&mut self) -> &mut usize;
    fn busy_slots_mut(&mut self) -> &mut usize;
    fn cooling_slots_mut(&mut self) -> &mut usize;
    fn degraded_slots_mut(&mut self) -> &mut usize;
    fn challenge_open_slots_mut(&mut self) -> &mut usize;
    fn last_error_summary_mut(&mut self) -> &mut Option<String>;
    fn updated_at_mut(&mut self) -> &mut String;
}

impl BrowserExecutorHealthEntry for BrowserExecutorNodeHealthView {
    fn total_slots_mut(&mut self) -> &mut usize {
        &mut self.total_slots
    }
    fn warm_slots_mut(&mut self) -> &mut usize {
        &mut self.warm_slots
    }
    fn busy_slots_mut(&mut self) -> &mut usize {
        &mut self.busy_slots
    }
    fn cooling_slots_mut(&mut self) -> &mut usize {
        &mut self.cooling_slots
    }
    fn degraded_slots_mut(&mut self) -> &mut usize {
        &mut self.degraded_slots
    }
    fn challenge_open_slots_mut(&mut self) -> &mut usize {
        &mut self.challenge_open_slots
    }
    fn last_error_summary_mut(&mut self) -> &mut Option<String> {
        &mut self.last_error_summary
    }
    fn updated_at_mut(&mut self) -> &mut String {
        &mut self.updated_at
    }
}

impl BrowserExecutorHealthEntry for BrowserExecutorProviderHealthView {
    fn total_slots_mut(&mut self) -> &mut usize {
        &mut self.total_slots
    }
    fn warm_slots_mut(&mut self) -> &mut usize {
        &mut self.warm_slots
    }
    fn busy_slots_mut(&mut self) -> &mut usize {
        &mut self.busy_slots
    }
    fn cooling_slots_mut(&mut self) -> &mut usize {
        &mut self.cooling_slots
    }
    fn degraded_slots_mut(&mut self) -> &mut usize {
        &mut self.degraded_slots
    }
    fn challenge_open_slots_mut(&mut self) -> &mut usize {
        &mut self.challenge_open_slots
    }
    fn last_error_summary_mut(&mut self) -> &mut Option<String> {
        &mut self.last_error_summary
    }
    fn updated_at_mut(&mut self) -> &mut String {
        &mut self.updated_at
    }
}

fn update_health_entry<T: BrowserExecutorHealthEntry>(
    entry: &mut T,
    slot: &BrowserCapabilitySlotView,
    challenge_open: bool,
    degraded: bool,
    updated_at: String,
) {
    *entry.total_slots_mut() += 1;
    if matches!(
        slot.status,
        BrowserCapabilitySlotStatus::Warm | BrowserCapabilitySlotStatus::Hot
    ) {
        *entry.warm_slots_mut() += 1;
    }
    if slot.status == BrowserCapabilitySlotStatus::Busy {
        *entry.busy_slots_mut() += 1;
    }
    if slot.status == BrowserCapabilitySlotStatus::Cooling {
        *entry.cooling_slots_mut() += 1;
    }
    if degraded {
        *entry.degraded_slots_mut() += 1;
        let summary = slot
            .degradation_reasons
            .as_ref()
            .map(|reasons| reasons.join(", "))
            .filter(|value| !value.trim().is_empty());
        if summary.is_some() {
            *entry.last_error_summary_mut() = summary;
        }
    }
    if challenge_open {
        *entry.challenge_open_slots_mut() += 1;
    }
    let should_update_timestamp = {
        let current = entry.updated_at_mut().clone();
        updated_at.as_str() > current.as_str()
    };
    if should_update_timestamp {
        *entry.updated_at_mut() = updated_at;
    }
}

pub(super) fn is_challenge_open(slot: &BrowserCapabilitySlotView) -> bool {
    slot.degradation_reasons
        .as_ref()
        .map(|reasons| {
            reasons
                .iter()
                .any(|reason| reason.to_lowercase().contains("challenge"))
        })
        .unwrap_or(false)
}
