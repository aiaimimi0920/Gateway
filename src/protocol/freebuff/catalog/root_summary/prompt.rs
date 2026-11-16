//! Fixed Apache-2.0 summary instruction adaptation; see root_builder/README.md.
pub(super) fn instruction(target_tokens: u64) -> String {
    format!(
        r#"Stop working on the task. Write a structured summary of this whole conversation.
The summary replaces the conversation history. Another agent continues the work from it, with the same system prompt and tools, so it must be able to continue without the original messages.

Summarize what the user and the assistant said and did. Do not restate the system prompt, repository instructions or environment details: the next agent gets them separately. If the conversation starts with an earlier summary, merge it into this one. Newer messages take precedence over it.

Use exactly this format. Write "(none)" for an empty section.

## Objective
- [one or two sentences: what the user wants to accomplish]

## Requirements
- [constraints, preferences and scope limits the user stated; keep their exact wording when it matters]

## Decisions
- [decisions made and why]

## Work State
### Completed
- [finished items relevant to the objective, including from earlier summaries. Name each outcome and any check that confirmed it]
### Active
- [for each unfinished item, say what has already changed and what remains to do or verify]
### Blocked
- [what blocks progress, and why]

## Next Move
1. [the next actions, in order]

## Relevant Files
- `path`: [why it matters, what changed in it]

## Important Context
- [facts the next agent cannot easily find again: exact commands, error messages, values, identifiers, URLs, and test or build results]

## Latest Exchange
- [the user's latest message, and what the assistant last did or answered]

Rules:
- Use short single-line bullets, not paragraphs.
- Prefer short references over restating details. Leave out what the next agent can get again from the code or files.
- Keep exact file paths, symbols, commands, error text and identifiers.
- Keep user requests that are still open, with their exact wording. Drop the ones newer messages resolved.
- Keep the state of changes: uncommitted, committed, pushed, in review or merged.
- Name each image the user attached or a tool showed, by file name when it has one, with what it showed. The next agent cannot see any of them and must open the file again to look.
- Keep every heading exactly as written here, in English. Write the text under the headings in the language the user writes in.
- Keep the summary under about {target_tokens} tokens.
- Do not call tools. Do not continue the task. Return only the summary, with no preamble."#
    )
}
