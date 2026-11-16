//! Real SQLite/TCP canonical turn -> summary -> compacted work -> original FINISH.
use super::super::root_builder::tests::input;
use super::completion::activate;
use super::multistep::response;
use super::*;

pub(super) fn turn(history: &mut RootHistory) -> RootTurn<'_> {
    let step = input();
    RootTurn {
        context: step.context,
        history,
        tools: step.tools,
        stream: false,
    }
}
pub(super) fn summarized() -> String {
    "## Objective\n- Complete the synthetic task without repeating finished work.\n## Requirements\n- Preserve the current user's exact request.\n## Decisions\n- Use the original run.\n## Work State\n### Completed\n- First model step.\n### Active\n- Continue.\n### Blocked\n- (none)\n## Next Move\n1. Continue work.\n## Relevant Files\n- (none)\n## Important Context\n- Synthetic only.\n## Latest Exchange\n- Continue the original request.".into()
}
pub(super) fn usage() -> Value {
    let mut body = response(Some(0.125));
    body["usage"]["prompt_tokens"] = json!(20000);
    body["usage"]["completion_tokens"] = json!(10);
    body
}
#[tokio::test]
async fn freebuff_root_summary_wire_automatic_compaction_keeps_fresh_user_and_original_run() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let mut catalog = tests::body();
    catalog["rows"][0]["compaction"] =
        json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000,"maxContextLength":10000});
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    let old: Vec<_> = (0..12).map(|i| json!({"role":if i%2==0 {"user"} else {"assistant"},"content":format!("old-{i} {}", "detail ".repeat(30))})).collect();
    let mut history = RootHistory::new(old, 32768).unwrap();
    let fresh = json!({"role":"user","content":"current request must stay verbatim","host_extension":{"keep":true}});
    history.begin_turn(fresh.clone()).unwrap();
    peer.push(fixture::Spec::json(200, json!({"runId":"summary-run"})));
    peer.push(fixture::Spec::json(200, usage()));
    let mut summary = response(Some(0.25));
    summary["choices"][0]["message"]["content"] = json!(summarized());
    summary["usage"]["prompt_tokens"] = json!(99999);
    summary["usage"]["completion_tokens"] = json!(10);
    peer.push(fixture::Spec::json(200, summary));
    peer.push(fixture::Spec::json(200, response(Some(0.125))));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let first = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await;
        let (run, _) = first.continue_run().unwrap();
        let decoded = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await;
        assert!(
            decoded.completion().is_ok(),
            "automatic summary must recover the next root step from its reported context limit"
        );
        assert!(
            !decoded.context_forecast().unwrap().calibrated,
            "summary usage is not a work-history receipt"
        );
        assert_eq!(history.messages().len(), 3);
        assert!(history.messages()[0]["content"]
            .as_str()
            .unwrap()
            .contains("<conversation_summary>"));
        assert_eq!(history.messages()[1], fresh);
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 3);
        assert_eq!(finished.totals.total_credits, 52);
    })
    .await;
    assert_eq!(row(&db).await, original);
    assert_eq!(peer.count(), 6);
    {
        let hits = peer.hits.lock().unwrap();
        let work: Value = serde_json::from_slice(&hits[2].body).unwrap();
        let summary: Value = serde_json::from_slice(&hits[3].body).unwrap();
        let next: Value = serde_json::from_slice(&hits[4].body).unwrap();
        assert_eq!(
            &summary["messages"].as_array().unwrap()[..work["messages"].as_array().unwrap().len()],
            work["messages"].as_array().unwrap()
        );
        assert_eq!(work["tools"], summary["tools"]);
        assert_eq!(summary["tools"], next["tools"]);
        assert_eq!(summary["model"], work["model"]);
        assert_eq!(summary["codebuff_metadata"]["run_id"], "summary-run");
        assert_eq!(next["codebuff_metadata"]["run_id"], "summary-run");
        assert_eq!(summary["codebuff_metadata"]["compaction_summary"], "1");
        assert!(next["codebuff_metadata"]
            .get("compaction_summary")
            .is_none());
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        for hit in &hits[2..5] {
            business_tests::verify(hit, &public, "synthetic-fetch");
        }
        let finish: Value = serde_json::from_slice(&hits[5].body).unwrap();
        assert_eq!(finish["runId"], "summary-run");
        assert_eq!(finish["totalSteps"], 3);
        assert_eq!(finish["directCredits"], 52);
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

pub(super) fn scoped_account() -> (CatalogSession, DeviceRegistration) {
    let (mut session, device) = account();
    let mut catalog = tests::body();
    catalog["rows"][0]["compaction"] =
        json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000,"maxContextLength":10000});
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    (session, device)
}
pub(super) fn canonical() -> RootHistory {
    let old = (0..12)
        .map(|i| json!({"role":if i%2==0 {"user"} else {"assistant"},"content":format!("old-{i}")}))
        .collect();
    let mut history = RootHistory::new(old, 32768).unwrap();
    history
        .begin_turn(json!({"role":"user","content":"exact live request"}))
        .unwrap();
    history
}
