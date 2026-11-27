//! A timed-out management grant does not discard a verified login. Only pre-commit custody can resume.
use super::{save, Consent, Session};
use crate::{protocol::cursor_auth::Material, state::AppState};
use std::sync::Arc;

pub(super) async fn save_verified(
    state: &AppState,
    session: &Session,
    consent: Consent,
    material: Material,
) {
    let mut consent = Arc::new(consent);
    let mut renew = session.renew.subscribe();
    let mut stop = session.stop.subscribe();
    loop {
        if *stop.borrow() {
            return;
        }
        match save::persist(state, session, &consent, &material).await {
            Ok(()) => return,
            Err(error) if error.code.as_deref() == Some("console_secret_access_required") => {
                {
                    let mut view = session.inner.lock();
                    if *stop.borrow() {
                        return;
                    }
                    view.status = "awaiting_permission";
                    view.error_code = Some("console_secret_access_required");
                }
                // Material remains only in this bounded task, never in a GET, browser storage or second file.
                let changed = tokio::select! { biased;
                    _ = state.shutdown.wait() => { session.cancel(); return; },
                    _ = stop.changed() => return,
                    _ = tokio::time::sleep_until(session.deadline) => { session.fail("cursor_login_expired"); return; },
                    result = renew.changed() => result,
                };
                if changed.is_err() {
                    session.fail("cursor_import_interrupted");
                    return;
                }
                let next = renew.borrow_and_update().clone();
                session.renew.send_replace(None);
                renew.borrow_and_update();
                let Some(next) = next else {
                    session.fail("cursor_import_interrupted");
                    return;
                };
                consent = next;
            }
            Err(error) => {
                let code = match error.code.as_deref() {
                    Some("console_revision_conflict") => "console_revision_conflict",
                    Some("cursor_import_identity_conflict") => "cursor_import_identity_conflict",
                    Some("cursor_token_expired") => "cursor_token_expired",
                    Some("cursor_login_expired") => "cursor_login_expired",
                    _ => "cursor_import_save_failed",
                };
                session.fail(code);
                return;
            }
        }
    }
}
