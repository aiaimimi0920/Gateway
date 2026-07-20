#[cfg(not(feature = "line-accio-web-reverse-api"))]
mod tests {
    use crate::upstream::accio;

    #[test]
    fn unsupported_accio_request_plan_reports_compiled_out() {
        let error = accio::unsupported_request_plan_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("Accio"));
    }
}
