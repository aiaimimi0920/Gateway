use std::collections::HashMap;

use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeminiCanvasDirectHttpApiKeyTransport {
    HeaderOnly,
    QueryOnly,
    HeaderAndQuery,
}

impl GeminiCanvasDirectHttpApiKeyTransport {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::HeaderOnly => "header",
            Self::QueryOnly => "query",
            Self::HeaderAndQuery => "header+query",
        }
    }

    pub(crate) fn include_header(self) -> bool {
        matches!(self, Self::HeaderOnly | Self::HeaderAndQuery)
    }

    pub(crate) fn include_query(self) -> bool {
        matches!(self, Self::QueryOnly | Self::HeaderAndQuery)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeminiCanvasPageHarvestMode {
    AnonymousLightweightPlain,
    AnonymousLightweightEmulated,
    SessionLightweightPlain,
    SessionLightweightEmulated,
    SessionBrowserNavigationEmulated,
}

impl GeminiCanvasPageHarvestMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::AnonymousLightweightPlain => "anon-light-nav-plain",
            Self::AnonymousLightweightEmulated => "anon-light-nav-emulated",
            Self::SessionLightweightPlain => "session-light-nav-plain",
            Self::SessionLightweightEmulated => "session-light-nav-emulated",
            Self::SessionBrowserNavigationEmulated => "session-browser-nav-emulated",
        }
    }

    pub(crate) fn use_plain_http(self) -> bool {
        matches!(
            self,
            Self::AnonymousLightweightPlain | Self::SessionLightweightPlain
        )
    }

    pub(crate) fn include_cookie_header(self) -> bool {
        matches!(
            self,
            Self::SessionLightweightPlain
                | Self::SessionLightweightEmulated
                | Self::SessionBrowserNavigationEmulated
        )
    }

    pub(crate) fn include_locale_header(self) -> bool {
        self.include_cookie_header()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GeminiCanvasRuntimeApiContext {
    pub(crate) payload: ProviderAccountPayload,
    pub(crate) api_key_candidates: Vec<String>,
    pub(crate) session: gemini_canvas::GeminiCanvasPureHttpSession,
    pub(crate) page_origin: String,
    pub(crate) page_referer: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GeminiCanvasProgramAppEndpointApiContext {
    pub(crate) relay_config:
        crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
    pub(crate) runtime_api: GeminiCanvasRuntimeApiContext,
    pub(crate) official_extra_headers: HashMap<String, String>,
    pub(crate) invoke_base_url: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GeminiCanvasSignalerChannel {
    pub(crate) api_key: String,
    pub(crate) gsession_id: String,
    pub(crate) sid: String,
    pub(crate) next_aid: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_transport_flags(
        transport: GeminiCanvasDirectHttpApiKeyTransport,
        include_header: bool,
        include_query: bool,
    ) {
        assert_eq!(transport.include_header(), include_header);
        assert_eq!(transport.include_query(), include_query);
    }

    #[test]
    fn gemini_canvas_direct_http_api_key_transport_helpers_match_expected_modes() {
        assert_eq!(
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly.label(),
            "header"
        );
        assert_transport_flags(
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            true,
            false,
        );

        assert_eq!(
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly.label(),
            "query"
        );
        assert_transport_flags(
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
            false,
            true,
        );

        assert_eq!(
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery.label(),
            "header+query"
        );
        assert_transport_flags(
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
            true,
            true,
        );
    }

    #[test]
    fn gemini_canvas_page_harvest_mode_helpers_match_expected_flags() {
        assert_eq!(
            GeminiCanvasPageHarvestMode::AnonymousLightweightPlain.label(),
            "anon-light-nav-plain"
        );
        assert!(GeminiCanvasPageHarvestMode::AnonymousLightweightPlain.use_plain_http());
        assert!(!GeminiCanvasPageHarvestMode::AnonymousLightweightPlain.include_cookie_header());
        assert!(!GeminiCanvasPageHarvestMode::AnonymousLightweightPlain.include_locale_header());

        assert_eq!(
            GeminiCanvasPageHarvestMode::SessionLightweightEmulated.label(),
            "session-light-nav-emulated"
        );
        assert!(!GeminiCanvasPageHarvestMode::SessionLightweightEmulated.use_plain_http());
        assert!(GeminiCanvasPageHarvestMode::SessionLightweightEmulated.include_cookie_header());
        assert!(GeminiCanvasPageHarvestMode::SessionLightweightEmulated.include_locale_header());

        assert_eq!(
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated.label(),
            "session-browser-nav-emulated"
        );
        assert!(
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated.include_cookie_header()
        );
        assert!(
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated.include_locale_header()
        );
    }
}
