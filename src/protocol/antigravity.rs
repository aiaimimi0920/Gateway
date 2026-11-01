//! Fixed endpoints and public desktop OAuth parameters; personal tokens never belong here.
mod oauth_client;
pub use oauth_client::OAuthClient;
pub const ADAPTER: &str = "antigravity_oauth_compatible";
pub const BASE_URL: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal";
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
// Public installed-client parameters, not user tokens. Exact line-scoped scanner exceptions.
pub const CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com"; // gitleaks:allow -- public desktop OAuth client parameter
pub const CLIENT_SECRET: &str = "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"; // gitleaks:allow -- public desktop OAuth client parameter
pub const REDIRECT_URI: &str = "http://localhost:51121/oauth-callback";
pub const SCOPES: &str = "openid https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/cclog https://www.googleapis.com/auth/experimentsandconfigs https://www.googleapis.com/auth/aicode";
