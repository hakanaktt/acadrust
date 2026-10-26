# Instructions for automated contributors

This file is for coding agents and other automated tools that open issues or pull requests against this repository. Humans should read `CONTRIBUTING.md`, which has the same rules with more explanation.

## Reporting a bug

1. File an issue with the bug template. The body needs these four headings, in this order: `## What was expected`, `## What actually happened`, `## Proposed solution`, `## Alternatives`.
2. Open a **draft** PR immediately, with `Closes #N` in the body.
3. The first commit on that PR contains only tests, and they fail (red) for the reason the issue describes. Push it and confirm they fail before you write any fix.
4. Add the fix as a **second commit on the same PR**. Don't open a separate PR for the fix, and don't squash or rewrite the red commit.
5. Mark the PR ready only when the new tests are green and `cargo test` passes.

## Don't

- Submit a fix with no earlier red-tests commit.
- Write the tests after the fix and present them as a reproduction.
- Open one PR for the tests and another for the fix.
- Bundle unrelated refactors or formatting-only changes.
- Open a PR without a closing keyword (`Closes #N`) for the issue it resolves.
- Open many near-identical issues or PRs in one go. One report, one draft PR.

PRs that skip the red-first flow will be closed and asked to be resubmitted.
