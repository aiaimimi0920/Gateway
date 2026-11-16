//! Feature-off keeps parsing/recognition available but contains no HTTP grant sender.
use super::{CodeGrant, OAuthError, RefreshGrant, TokenMaterial};

impl CodeGrant {
    pub async fn exchange(self, _http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        Err(OAuthError::FeatureDisabled)
    }
}
impl RefreshGrant {
    pub async fn execute(self, _http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        Err(OAuthError::FeatureDisabled)
    }
}

#[cfg(test)]
#[tokio::test]
async fn accio_feature_disabled_never_sends_grant_material() {
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    let refresh = RefreshGrant::new("synthetic-access", "synthetic-refresh", None).unwrap();
    assert_eq!(
        refresh.execute(&http).await.unwrap_err(),
        OAuthError::FeatureDisabled
    );
    let auth = super::AuthorizationAttempt::new(
        super::OAuthClient::new("synthetic-client").unwrap(),
        super::LoginSite::China,
        12345,
    )
    .unwrap();
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let callback = format!("{}?code=synthetic-code&state={state}", auth.redirect_uri());
    assert_eq!(
        auth.finish(&callback)
            .unwrap()
            .exchange(&http)
            .await
            .unwrap_err(),
        OAuthError::FeatureDisabled
    );
}
