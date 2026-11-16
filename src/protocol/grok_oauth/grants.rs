//! Code/device acceptance retains the difference between signed user identity and endpoint delivery.
use super::{
    Authorization, DeviceChallenge, DevicePoll, GrantFlow, GrantPoll, GrantReceipt, OAuthClient,
    OAuthError, PrincipalConstraint, PrincipalHint,
};

impl OAuthClient {
    pub async fn complete_code(
        &self,
        http: &rquest::Client,
        auth: &Authorization,
        callback: &str,
        constraint: Option<&PrincipalConstraint>,
    ) -> Result<GrantReceipt, OAuthError> {
        if auth.client != *self {
            return Err(OAuthError::InvalidInput);
        }
        let code = auth.callback_code(callback)?;
        let tokens = self.exchange_code(http, auth, &code).await?;
        let mut receipt = GrantReceipt::new(self, tokens, GrantFlow::Code)?;
        receipt.principal = PrincipalHint::read(&receipt.tokens.access_token)?;
        if let Some(constraint) = constraint {
            constraint.check(receipt.principal.as_ref())?;
        }
        // The issuer can strip openid when Team is chosen at consent. Pre-selection is not proof
        // of what was chosen. Team accepts endpoint delivery without claiming verified user identity.
        if !receipt
            .principal
            .as_ref()
            .is_some_and(PrincipalHint::is_team)
        {
            receipt.identity = Some(
                self.verify_code_identity(http, auth, &receipt.tokens)
                    .await?,
            );
        }
        if receipt.is_expired() {
            return Err(OAuthError::Expired);
        }
        Ok(receipt)
    }
    pub async fn poll_device_receipt(
        &self,
        http: &rquest::Client,
        challenge: &mut DeviceChallenge,
        constraint: Option<&PrincipalConstraint>,
    ) -> Result<GrantPoll, OAuthError> {
        let tokens = match self.poll_device(http, challenge).await? {
            DevicePoll::Pending => return Ok(GrantPoll::Pending),
            DevicePoll::SlowDown => return Ok(GrantPoll::SlowDown),
            DevicePoll::Authorized(tokens) => tokens,
        };
        let mut receipt = GrantReceipt::new(self, tokens, GrantFlow::Device)?;
        receipt.principal = PrincipalHint::read(&receipt.tokens.access_token)?;
        if let Some(constraint) = constraint {
            constraint.check(receipt.principal.as_ref())?;
        }
        // Device has no browser nonce. Optional ID-token display claims are not a VerifiedIdentity.
        if receipt.is_expired() {
            return Err(OAuthError::Expired);
        }
        Ok(GrantPoll::Authorized(receipt))
    }
}
