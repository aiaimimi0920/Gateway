//! Browser reauthorization is a new durable family, including when the provider repeats tokens.
use super::{lobsterai_fixture::*, *};
#[tokio::test]
async fn lobsterai_reauthorization_escapes_old_sending_and_published_fences_without_erasing_history(
) {
    for published in [false, true] {
        let f = fixture().await;
        let same = json!({"code":0,"data":{"accessToken":"synthetic-access"}});
        let peer = Peer::start(if published {
            vec![
                Reply::json(same.clone()),
                Reply::json(same.clone()),
                Reply::json(same),
            ]
        } else {
            vec![Reply::json(same.clone()), Reply::json(same)]
        })
        .await;
        ready(&f, &peer).await;
        let old = request(&f);
        if published {
            reject(&f);
            refresh(&f, &peer, &mut Pending::new()).await;
        } else {
            reserve(&f, &old).await;
        }
        let snapshot = f.runtime.route_config().snapshot();
        let mut doc = snapshot.document().clone();
        let c = &mut doc.providers[0].credentials[0];
        let prior = c.lobsterai.as_ref().unwrap();
        let initial =
            ReceivedTokens::from_json(protocol::test_support::tokens().to_string().as_bytes())
                .unwrap()
                .admit_initial(prior.installation_id())
                .unwrap();
        c.lobsterai = Some(prior.reauthorization(&initial.context).unwrap());
        f.runtime
            .commit_document(snapshot.revision().id(), doc, None)
            .await
            .unwrap();
        ready(&f, &peer).await;
        let next = request(&f);
        assert_ne!(old.family_id(), next.family_id());
        for _ in 0..2 {
            reject(&f);
            refresh(&f, &peer, &mut Pending::new()).await;
            owner(&f).dispatch_readiness().unwrap();
        }
        assert_eq!(
            load(&f, &old).phase(),
            if published {
                GrantPhase::Published
            } else {
                GrantPhase::Sending
            }
        );
        assert_eq!(load(&f, &next).phase(), GrantPhase::Published);
        peer.inspect(|seen| assert_eq!(seen.len(), if published { 3 } else { 2 }));
    }
}
