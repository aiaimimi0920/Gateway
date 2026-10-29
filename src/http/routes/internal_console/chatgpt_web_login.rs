//! One bounded interactive login, with explicit cancellation independent of HTTP disconnects.
use crate::upstream::ConsoleBrowserInteraction;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::watch;

static ACTIVE: Mutex<Option<ActiveLogin>> = Mutex::new(None);
// A bounded tombstone queue handles a cancel request arriving before its login request.
static CANCELLED: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

#[derive(Clone, PartialEq, Eq)]
pub(super) struct LoginOwner {
    pub actor: String,
    pub provider: String,
    pub revision: String,
}

struct ActiveLogin {
    id: String,
    owner: LoginOwner,
    sender: watch::Sender<bool>,
    interaction: ConsoleBrowserInteraction,
}

pub(super) struct LoginGuard(String);

impl LoginGuard {
    pub(super) fn begin(
        id: String,
        owner: LoginOwner,
        interaction: ConsoleBrowserInteraction,
    ) -> (Self, watch::Receiver<bool>) {
        let (sender, receiver) = watch::channel(false);
        let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        let cancelled = CANCELLED.lock().unwrap_or_else(|e| e.into_inner());
        if cancelled.contains(&id) {
            let _ = sender.send(true);
        }
        *active = Some(ActiveLogin {
            id: id.clone(),
            owner,
            sender,
            interaction,
        });
        (Self(id), receiver)
    }
}

impl Drop for LoginGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|login| login.id == self.0) {
            active.take();
        }
    }
}

pub(super) fn cancel(id: &str) {
    let active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(login) = active.as_ref() {
        if login.id == id {
            let _ = login.sender.send(true);
        }
    }
    let mut cancelled = CANCELLED.lock().unwrap_or_else(|e| e.into_inner());
    if cancelled.len() == 32 {
        cancelled.pop_front();
    }
    cancelled.push_back(id.to_owned());
}

pub(super) fn interaction(id: &str, owner: &LoginOwner) -> Option<ConsoleBrowserInteraction> {
    let active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    active
        .as_ref()
        .filter(|login| login.id == id && &login.owner == owner)
        .map(|login| login.interaction.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_is_scoped_and_guard_clears_registration() {
        let workspace = crate::upstream::ConsoleBrowserWorkspace::create()
            .await
            .unwrap();
        let owner = LoginOwner {
            actor: "actor".into(),
            provider: "provider".into(),
            revision: "revision".into(),
        };
        let (guard, receiver) =
            LoginGuard::begin("selected".into(), owner.clone(), workspace.interaction());
        assert!(interaction("selected", &owner).is_some());
        assert!(interaction("other", &owner).is_none());
        for other in [
            LoginOwner {
                actor: "other".into(),
                ..owner.clone()
            },
            LoginOwner {
                provider: "other".into(),
                ..owner.clone()
            },
            LoginOwner {
                revision: "other".into(),
                ..owner.clone()
            },
        ] {
            assert!(interaction("selected", &other).is_none());
        }
        cancel("other");
        assert!(!*receiver.borrow());
        cancel("selected");
        assert!(*receiver.borrow());
        drop(guard);
        assert!(ACTIVE.lock().unwrap().is_none());
        cancel("early");
        let (_guard, receiver) = LoginGuard::begin("early".into(), owner, workspace.interaction());
        assert!(*receiver.borrow());
    }
}
