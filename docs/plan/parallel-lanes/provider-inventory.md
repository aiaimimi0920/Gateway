# Lane O: provider inventory generator decomposition

Coordinator owns tools/generate-gateway-provider-inventory.py, its new
tools/provider_inventory package and directly affected inventory contract tests.
Rust source and Cargo remain read-only metadata inputs, owned by S06.

Baseline: 749 effective lines; 11 inventory contract tests pass. The extraction
will separate the canonical/path/error contract, metadata parsing/provenance,
and evidence redaction/path normalization. The entry retains manifest discovery,
inventory assembly, evidence orchestration and CLI/output handling. All completed
owners must be below 500 effective lines without a baseline or exception change.

Preserved contracts include 44 canonical provider lines, deterministic output,
source joins, explicit evidence validation, Gateway-relative artifact paths,
secret scrubbing, schema/CLI exit behavior and test-injected manifest discovery.
Moved function bodies will be compared against the saved baseline. Direct CLI
and dynamic test loading must both resolve the extracted package. No live
provider, credential sample, Cargo invocation or release directory is in scope.

Evidence root: target/effective-line-evidence/provider-inventory. Structural
acceptance, integration and release inclusion remain pending implementation.

## Structural acceptance

Generator is now 411 effective lines, with contracts 68, metadata 228 and
redaction 81. The package initializer contains only its package description.
Repository-root resolution changes from entry parents[1] to package parents[2],
pointing to the same Gateway directory. The test's dynamic file loader temporarily
adds the tool directory during module execution, matching direct CLI discovery
without permanently changing sys.path. Manifest discovery remains in the entry,
so the existing missing-canonical-line injection contract still exercises it.

All 30 original function ASTs are unchanged and uniquely owned. A deterministic
44-line inventory from the new generator exactly equals the saved baseline and
validates against the checked-in schema. All source owners are UTF8 without BOM.
Verification: original inventory 11/11 before and after; related provider-evidence
integration 9/9; Python compilation, ratchet and diff pass. Independent read-only
review found no missed dependency/root/callsite regression. No repository Python
formatter configuration was found among pyproject.toml, ruff.toml, .ruff.toml or
setup.cfg; the extraction preserves function formatting.

The existing nested package fixture now copies the real generator and its four
package files, verifies every byte/checksum/support record and executes the
packaged generator's --help from outside its tool directory. This proves package
import resolution and CLI option exposure; the fixture passed 1/1. The packager's
existing recursive tools copy includes the new package; production packaging
code was not modified. No full release or provider access was involved.

Strict snapshot: 1143 files, 40 hard and 73 mandatory failures, 38 soft files;
strict remains nonzero for inherited debt. Baseline/exception registries are
unchanged. Structural debt clearance advances to 32/145, with 113 files still
above 700 effective lines. Evidence: before-tests.log, after-tests.log,
equivalence.log, runner-integration.log, package.log, ratchet.log, strict.log,
diff.log and verify-split.py under this lane's evidence root.

This is structural acceptance with focused integration proof. Existing regex
source parsers, recursive sanitization, unbounded operator-supplied evidence and
the generator's non-atomic output writer were preserved, not certified as fully
hardened. Actual immutable release inclusion awaits GWP-20260908-06; the earlier
full 283-test Python checkpoint predates this lane.

Superseding integration checkpoint: Lane P's full offline Python run includes
this extraction and passes 283 tests in 665.214s with four explicit skips.
Actual immutable release inclusion is still pending the shared build window.
