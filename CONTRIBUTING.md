# Contributing

Thanks for sending something in. These rules apply to everyone, and they apply in full to automated and AI-assisted contributors (coding agents, bots, scripted PR tools). Those are the contributors most likely to send a fix with no proof the bug was real, so the flow below is written with them in mind.

## Bugs: issue first, then a draft PR with red tests

1. **Open an issue** with these four headings, in this order, all filled in:
   - `## What was expected`
   - `## What actually happened`
   - `## Proposed solution`
   - `## Alternatives` (if there really is none, say so and why)
2. **Open a draft PR straight away** with `Closes #N` in the body. Its first commit adds the tests that reproduce the report, and they must be **red** when you push. One smoke test is not enough, I want the set that covers what the issue claims.
3. **Push the fix as a separate commit on the same PR.** Don't open a second PR for the fix, and don't squash the red commit away. The history (red commit, then fix commit) is what shows the fix addressed the real cause and not something nearby.
4. Mark the PR ready once the fix commit turns those tests green and the rest of `cargo test` still passes.

A PR whose tests were written after the fix, or whose tests went green on the first run, hasn't shown anything and will be sent back.

## Features and other changes

Open an issue first and reference it with `Closes #N`. Tests go in with the change. If it changes behaviour that is already tested, say which tests moved and why.

## PR checklist

- [ ] The PR body contains `Closes #N`.
- [ ] For a bug: the red-tests commit comes first, and the description says which commit is which.
- [ ] `cargo test` passes on the final commit, and the description says what you ran.
- [ ] No formatting-only hunks in files you didn't otherwise change.
- [ ] No unrelated changes.

## Automated contributors

Read `AGENTS.md` too. The rules above aren't optional: no red commit, no review.
