# Accio OAuth wire reference

- Source: [MYCF711/magpie-accio-auth](https://github.com/MYCF711/magpie-accio-auth/tree/62993d5d7dc872ea4f62ad50116b698c62ea513a)
- Pinned commit: `62993d5d7dc872ea4f62ad50116b698c62ea513a`
- Consulted files: `src/constants.mjs`, `src/login.mjs`, `src/credential.mjs`, `LICENSE`.
- License: MIT; the original notice is retained in `LICENSE` beside this file.
- Adoption: independent Rust implementation of the PKCE parameters, fixed JSON exchange/refresh
  shapes, envelope fields and numeric expiry units. No invitation/referral, error-prose matching,
  unverified JWT identity, local-time date parsing or fallback authorization lifetime is adopted.
- This is a community wire reference, not an official integration/third-party-client license.
  Gateway does not embed the reference's `accio-work` client ID. The operator must supply an
  authorized client. Login-site selection does not prove independent or shared quota.
- The protocol owner does not create a callback listener, management session, saved credential,
  account identity, `utdid`, or quota pool. Those remain separate framework responsibilities.
