---
name: pr-review
description: Review a GitHub pull request or branch diff for actionable bugs, regressions, and missing tests. Use when asked to review a PR, MR, pull request, or code changes before merge.
---

# MR / PR Review

Review the proposed changes as a code reviewer. Optimize for finding real defects that should change the author's decision to merge, not for producing a large number of comments.

## Establish the review target

Determine the base and head of the change without modifying the working tree.

- If the user provides a GitHub PR URL or number and `gh` is available, use GitHub PR metadata and diff as the source of truth. Read the PR title/body when they help establish intent.
- If the current branch has a GitHub PR, `gh pr view` / `gh pr diff` may be used to obtain the same context.
- Otherwise review the local branch against its appropriate base branch using Git. Prefer the repository's configured default/upstream branch over assuming `main`.
- Honor repository instructions such as `AGENTS.md` and relevant project documentation.
- Do not checkout branches, rewrite files, commit, push, post comments, approve, or request changes unless the user explicitly asks.

Useful commands when applicable:

```sh
gh pr view <pr> --json number,title,body,baseRefName,headRefName,files,commits
gh pr diff <pr>
git status --short
git diff --stat <base>...HEAD
git diff <base>...HEAD
```

Use commands selectively; do not run all of them when the target is already clear.

## Review method

Read the changed code and enough surrounding code to understand its behavior. Trace affected call sites, data flow, invariants, interfaces, and tests when necessary to validate a concern.

Prioritize issues involving:

- verify implementation against spec
- incorrect behavior or broken requirements
- regressions in existing behavior
- tests that are missing specifically because the changed behavior would otherwise be unprotected
- data loss, corruption, duplication, or non-idempotent behavior
- API, schema, serialization, or backward-compatibility breaks
- security or authorization mistakes
- concurrency, race, ordering, or transaction problems
- resource leaks or materially harmful performance changes

Avoid speculative findings. Before reporting a finding, confirm that the problematic path is reachable and that the repository does not already handle the condition elsewhere.

When useful, run focused existing tests or static checks to verify a suspected defect. Keep validation proportional to the changed area. Do not change implementation code during a review unless the user also asks for fixes.

## Finding quality bar

Each finding must be actionable and specific:

1. Identify the concrete behavior that is wrong.
2. State the conditions required to trigger it.
3. Explain the impact.
4. Point to the smallest useful changed line or line range.
5. Suggest the direction of a fix when it is not obvious.

Use these priorities:

- **P0** — catastrophic or broadly unsafe; merge must be blocked immediately.
- **P1** — high-impact bug or regression likely to affect normal production use; should block merge.
- **P2** — real defect with narrower conditions or impact; should be fixed before merge when practical.
- **P3** — low-impact but concrete correctness/robustness issue; optional unless project standards require it.

Do not inflate severity. Prefer a smaller set of high-confidence findings over exhaustive commentary.

## Output

Start with findings, ordered by severity. For each finding use:

```text
[P1] Short imperative title — path/to/file.ext:123
Explain the failure mode, trigger, and impact in one compact paragraph. Include a fix direction if useful.
```

Use the exact changed line when possible; otherwise cite the narrowest relevant line range. Do not cite an entire function when one or two lines identify the issue.

After findings, optionally include a short **Validation** section for tests/checks actually run and a short **Open questions** section only when unresolved context materially affects merge safety.

If there are no actionable findings, say:

```text
No actionable findings.
```

Then briefly state any meaningful validation gaps, such as tests you could not run. Do not invent issues to avoid returning a clean review.
