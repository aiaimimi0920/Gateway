# ChatGPT HTML classification ownership and MIME

Status: verified, coordinator-owned, 2026-09-15.

Verified predecessor: SSE MIME scope 7754e92e8a41083c9c851dc46c67db354a65af4c4818e89a13fee238f8882d49.
Scope: protocol/chatgpt/web_reverse/response.rs and new response/classification.rs
and response/tests.rs. Existing response owner measures 652 effective lines;
production portion 462, classifier functions 62 and test module 190.

## Original plan

First run the original broad library chatgpt filter. Extract classifier ownership
and tests with unchanged production functions and public exports; append four
negative MIME groups and preservation tests. Run the same filter with the new
regressions before fixing the browser challenge predicate. Finally compare only
the HTML essence (ASCII case-insensitive, SP/HTAB trim, first semicolon boundary).
Session-invalid logic, status/marker predicates, errors and stream translation stay
unchanged. Original function/test source projections are checked with rustfmt.

All source/test/assets remain frozen during serialized native gates. Close with
all-targets, scoped formatter, checker tests, ratchet, strict inventory, both Git
checks and independent review. New and completed owners must remain <=500 lines.

False challenge codes can trigger request-time browser refresh and relay; test the
public classifier code/provider/status contract as well as direct detection.
True HTML content sniffing, true challenges and invalid-session handling must stay
compatible. Full MIME parameter syntax validation is outside this predicate.
Synchronous solver scheduling and full product/release acceptance remain open.
S06 original implementation, cursor and final-build coordination stay reserved.

## Verified result

Original 163/163; regression 166 passed / 4 failed; candidate 170/170. Original
identities and three preservation cases stay green. Entry 652 -> 404;
classifier/tests 72/316. All-targets/scoped rustfmt/source/checker 19/19/ratchet/Git
checks pass. Strict 2094/12/20/39; clearance 113/145 (77.9%). Exact formatted
projections retain all original functions and assertions except the intended
browser-header predicate. Before-source CRLF and failed formatter receipts remain
recorded; no executed regression test was changed afterward.
Scope SHA-256: 9ab51c1137bf094edf1abd13f3a801d1f11edbf336757a7250c27bfa63ddb4ea.
[Full checkpoint](../../status/2026-09-15-chatgpt-html-mime.md).
