// Adapted from CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d.
// Copyright 2025 Codebuff. Apache-2.0; see LICENSE, NOTICE and README.md here.
// Modifications: Rust rendering, trusted explicit host context, one-pass substitution.
pub(super) fn lean(max_context: u64) -> String {
    // Match the fixed JS builder's floor/round, including its IEEE-754 arithmetic.
    let threshold = (max_context as f64 * 0.8).floor();
    let condensed_at = (threshold / 1000.0).round() as u64;
    format!(
        "\n{}\n",
        super::prompt::render(LEAN, &[("CONDENSED_AT", &condensed_at.to_string())])
    )
}
pub(super) const BATCHING: &str = r###"## Work in batches

Every response is a round trip: the whole conversation is sent again before the next reply begins, so a task done in fewer responses finishes sooner. Get as much done in each response as you can.

Batching is about fewer round trips, never about doing less: do every part of the request and verify it.

- **Make every call you already can.** Tool calls that do not depend on each other's results belong in one response, not in a sequence of responses.
- **Search in one batch.** When you need to find something, send every code_search and glob you can think of together (the names, strings and paths it could be under), rather than one search per response.
- **Read in one batch.** Read every file that looks relevant in one read_files call, often 5 to 15 of them, including the ones you expect to need next, rather than one file at a time. Go back for more only when what you read points to something specific.
- **Explore with the dedicated tools.** code_search, glob, list_directory and read_files take many patterns or paths per call; ls, cat, grep and find in the terminal do not.
- **Edit in one batch.** Make all the edits you already know in one response.
- **Check in one batch.** Run the checks in the same response as your last edits, as one terminal command that runs every check the change needs (for example each touched package's typecheck and the tests that cover the change), each with its exit status echoed and not piped into head or tail.
- **Stop when it is done.** Once every part of the request is done and its checks pass, or the question is answered, give the final message."###;
const LEAN: &str = r###"# Context budget

Your context is small: everything you read or run stays in this conversation. When it grows past ~{{CONDENSED_AT}}k tokens it is condensed automatically: you keep the user's request, a log of your earlier messages and tool calls, the most recent file reads, and an outline with line numbers of the other files you read. Old tool output is gone after that, and files you edited are not carried over; re-read only the window you need, using the outline's line numbers.

The budget limits how you read, never how much of the task you do. Do every part of the request and verify it as thoroughly as you would with unlimited context.

- Search widely, read narrowly. code_search is cheap: use it to find every definition, caller, test and type the change touches (matches come with line numbers), then read the parts you need.
- Read a whole file only when it is about 100 lines or fewer. For a longer file, read a window with { path, offset, limit } of 100-250 lines around what you need, enough to see the surrounding function or block in full; avoid slices of a few dozen lines. The result tells you the file's total line count.
- Don't re-read lines you have already seen unless the file changed since.
- Keep terminal output short: filter long output (grep, head, tail -n 50) while preserving the exit status. Still run the typecheck and the tests that cover your change.
- Edit with str_replace rather than rewriting whole files.
- For any task with more than a couple of steps, write the plan with write_todos before editing and keep it current. Your remaining todos survive condensing, so they are how you pick up where you left off; without them you lose track of what is done.
- Stay within the request. After condensing, check your todos and the request before starting anything new rather than broadening the change.
- Before finishing, code_search for remaining references to anything you renamed, removed or changed the signature of, including tests, and update them."###;
