# FreeBuff CLI root prompt adaptation

These Rust modules adapt the default CLI root prompt and its conditional guidance
from `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`:
`agents/base3.ts`, `agents/constants.ts`, `agents/lean-context.ts`,
`common/src/constants/agent-work-style.ts`, `turn-ending-rule.ts`,
`base3-step-prompt.ts`, and `common/src/util/system-reminder.ts`.
The native history preparation also adapts the interrupted-call semantics in
`common/src/util/messages.ts`, the pre-reminder ordering in
`packages/agent-runtime/src/run-agent-step.ts`, and assistant-run merging in
`packages/llm-providers/src/openai-compatible/chat/convert-to-openai-compatible-chat-messages.ts`.

Copyright 2025 Codebuff. Upstream Apache-2.0 LICENSE and NOTICE are preserved in
this directory (NOTICE's one trailing ASCII space and missing final newline are normalized;
its copyright/attribution text is unchanged).
The rest of Gateway keeps its existing license. Modifications:
Rust-only rendering, explicit trusted host context, single-pass substitution,
same-snapshot catalog identity, native request assembly and existing run ownership.
History-specific adaptations: native `tool_calls`/`tool_call_id` pairing, borrowed-ID
reverse scan, bounded raw-snapshot preflight, owned/amortized-linear merging,
preservation of freestanding results and native non-text payloads, and local
rejection of nonempty mixed string/part-array assistant content. Native input is
not the SDK's internal part format: `providerExecuted` cannot authorize a call;
existing reasoning fields are retained, with no inferred model/signature provenance.
Other assistant metadata uses the fixed converter's shallow later-wins precedence.
No upstream TypeScript or SDK code is executed. This is an adaptation, not an
unmodified vendor asset; all Rust content is measured by the normal line policy.

This implements the default CLI root, not the noAskUser/noWeb/workspace variants.
It does not provide filesystem, terminal, web, UI, skill or other tool execution,
automatic summaries, token accounting, child agents, or public admission.
The internal host must implement the advertised tool contracts and prepare the
history with real context budgeting/compaction before using execute_root_step.
The builder repairs only its consumed snapshot, not a persistent canonical history;
it is not the unpublished web API's provider-specific orphan/duplicate sanitizer.
Prompt text describing those contracts is not evidence that Gateway executes them.

Subsequent `catalog/root_context` adapts the same fixed runtime's `util/token-counter.ts`
and `util/context-token-count.ts`: bounded cheap native estimates, provider-usage anchoring,
and a run-local runtime budget gate, not BPE or exact model token accounting.
History-prefix/system/tool witnesses reject receipt reuse after rewrites; generic harnesses
and different runs never share calibration. Wire-native schemas/arguments/reasoning are
adapted rather than converted to the SDK's internal parts. Missing usage is not zero.
This snapshot entry does not implement automatic summaries or persistent canonical history;
the host still owns the advertised tools. Source and modification
attribution continues under the preserved Apache-2.0 LICENSE/NOTICE above.
Only fresh bare native history snapshots are accepted by contract; generated
instructions/step reminders must not be persisted back into the host snapshot.

Subsequent `catalog/root_history`, `root_summary` and `execution/root_turn` adapt
the same fixed runtime's `conversation-summary.ts`, `compaction.ts`, `run-agent-step.ts`,
and `common/src/constants/compaction-summary.ts`/`compaction-policy.ts`:
foreground reported-usage threshold decisions, the structured-summary instruction,
reply extraction and memory wrapper, and a canonical in-memory turn history.
An exclusive borrow pins the covered history; a validated bounded candidate preserves
all current-turn user messages verbatim before replacing earlier exchanges. Summary
costs/cancellation share the original sequential run owner, not a parallel lifecycle.
Native prefix reuse and local byte/token/stop policies differ from SDK internal tags;
this does not implement background/idle/manual summaries, durable history or tools.
The source attribution and Apache-2.0 LICENSE/NOTICE above cover these adaptations too.

Source redistributions must retain LICENSE, NOTICE and this modification notice.
No binary/package is distributed in this sub-batch. Before a binary release,
P50 packaging must include these third-party notices with the distribution;
source compilation alone is not a package/license acceptance result.
