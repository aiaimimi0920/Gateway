#[path = "gemini_canvas_direct_http_contract_helpers.rs"]
mod contract_helpers;
#[path = "gemini_canvas_direct_http_error_helpers.rs"]
mod error_helpers;
#[path = "gemini_canvas_direct_http_json_helpers.rs"]
mod json_helpers;
#[path = "gemini_canvas_direct_http_page_harvest_helpers.rs"]
mod page_harvest_helpers;

pub(crate) use contract_helpers::*;
pub(crate) use error_helpers::*;
pub(crate) use json_helpers::*;
pub(crate) use page_harvest_helpers::*;

#[cfg(test)]
#[path = "gemini_canvas_direct_http_contract_bootstrap_tests.rs"]
mod direct_http_contract_bootstrap_tests;
#[cfg(test)]
#[path = "gemini_canvas_direct_http_contract_resolution_tests.rs"]
mod direct_http_contract_resolution_tests;
#[cfg(test)]
#[path = "gemini_canvas_direct_http_error_tests.rs"]
mod direct_http_error_tests;
#[cfg(test)]
#[path = "gemini_canvas_direct_http_json_tests.rs"]
mod direct_http_json_tests;
#[cfg(test)]
#[path = "gemini_canvas_direct_http_page_harvest_tests.rs"]
mod direct_http_page_harvest_tests;
#[cfg(test)]
#[path = "gemini_canvas_direct_http_test_support.rs"]
mod direct_http_test_support;
