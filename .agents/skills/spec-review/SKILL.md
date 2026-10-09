---
name: spec-review
description: Review product and feature specs for concise, explicit, testable behavior and intent. Use when asked to review a spec, requirements, acceptance criteria, behavioral RFC, or to compare an implementation against its specification.
---

# Spec Review

Review the specification as the source of truth for product behavior and intent. Code implements the spec; tests verify the implementation. Neither code nor tests should silently define requirements that the spec does not state.

Optimize for a spec that is short enough to understand quickly and precise enough that two engineers should implement materially the same behavior.

## Core principles

- Specify **observable behavior and intent**, not implementation structure.
- Explain **how the feature is used** from the user's perspective.
- State **business rules explicitly**: defaults, limits, validation, permissions, precedence, state transitions, and failure behavior.
- Prefer short declarative rules over descriptive paragraphs.
- Keep rationale only when it explains a non-obvious decision or constraint.
- Use one term for one concept. Avoid synonyms that create accidental distinctions.
- Do not hide requirements inside examples, notes, or implementation discussion.
- Do not use vague qualifiers such as "normally", "typically", "appropriate", "as needed", or "should handle" when a concrete rule is required.
- Preserve implementation freedom where implementation details do not affect the behavioral contract.

## Establish the review target

Read the complete spec and any repository instructions relevant to it. When needed, read adjacent specs to understand established terminology and existing business rules.

If implementation code already exists, do not use it to fill gaps in the spec unless the user explicitly asks to reconcile the two. An ambiguous spec remains ambiguous even if the current code chose one interpretation.

If the spec is marked draft, distinguish unresolved decisions from accidental ambiguity.

## Review method

Review for the smallest set of issues that materially weaken the spec as a behavioral contract.

### Usability

Verify that the spec makes the feature understandable from the user's point of view:

- who can use it
- how the user enters or invokes the workflow
- what inputs or choices are available
- what the user sees after each meaningful action
- what success looks like
- what failure, empty, loading, unavailable, or denied states look like when relevant
- whether actions are reversible, retryable, destructive, or persistent

Do not require UI mockup-level detail unless presentation is itself part of the contract.

### Business rules

Look for rules that an implementation would otherwise have to invent:

- eligibility and permissions
- required versus optional data
- defaults
- validation and limits
- precedence when multiple rules apply
- state transitions and lifecycle
- duplicate or repeated actions
- ownership and visibility
- time-based behavior
- deletion, retention, or restoration behavior
- error and retry semantics
- compatibility with existing behavior

Examples may illustrate a rule, but they do not replace stating the rule.

### Completeness and edge behavior

Trace the primary workflow and realistic alternate paths. Look specifically for unanswered questions where different reasonable implementations would produce different user-visible behavior.

Do not demand exhaustive enumeration of impossible or irrelevant edge cases. Require detail in proportion to product risk and ambiguity.

### Testability

Each normative statement should describe an outcome that can be verified.

Prefer:

```text
A cancelled invitation cannot be accepted.
```

Over:

```text
The system should properly handle cancelled invitations.
```

Acceptance criteria should derive from the spec's rules. They are not a substitute for the rules themselves.

### Concision

Treat unnecessary prose as a defect when it obscures the contract.

Recommend shortening when text:

- repeats a rule already stated elsewhere
- narrates obvious implementation behavior
- describes the feature without adding a decision, constraint, or rationale
- mixes rationale, implementation detail, and behavioral requirements in one paragraph
- uses several sentences where one direct rule would be clearer

Do not optimize for word count at the cost of precision.

## Reviewing implementation against a spec

When this skill is used during a PR review or explicitly to compare code with a spec, treat the spec as authoritative unless the user says it is stale or superseded.

Map changed behavior back to the relevant spec rules and classify mismatches correctly:

- **Spec is clear; implementation differs** → implementation defect.
- **Spec omits or ambiguously defines the behavior** → spec defect; do not guess intent from the code.
- **Implementation introduces material user-visible behavior not covered by the spec** → spec gap, even if the behavior appears reasonable.
- **Tests disagree with the spec** → tests or implementation are wrong; tests do not override the spec.
- **Spec and implementation intentionally changed together** → verify the spec still clearly states the resulting behavior and business rule.

A PR should not be considered behaviorally verified merely because its tests pass. Verify that the implementation and tests both reflect the written contract.

## Finding quality bar

Report only findings that improve the spec as a source of truth. A useful finding identifies one of these problems:

- contradictory requirements
- ambiguous behavior with multiple plausible implementations
- missing business rule
- missing user-visible state or workflow behavior
- requirement hidden only in an example or implementation note
- undefined interaction with existing behavior
- untestable or subjective requirement
- terminology that changes or obscures meaning
- excess prose that materially hides the actual rule

Use these priorities:

- **P1** — contradiction or ambiguity likely to produce materially incompatible implementations or incorrect product behavior.
- **P2** — missing rule, workflow state, or edge behavior that is likely to matter in normal use.
- **P3** — clarity, terminology, testability, or concision issue that makes the spec harder to use as a reliable contract.

Do not manufacture findings for completeness. Avoid line-editing prose unless the wording affects clarity or concision of the behavioral contract.

## Output

Start with findings, ordered by severity. Keep each finding compact.

For a file-backed spec, use the narrowest useful line reference:

```text
[P1] Define behavior for duplicate submissions — docs/spec.md:42
The spec allows submission but does not say what happens when the same request is submitted twice. That leaves implementations free to create duplicates or return the existing result. State the business rule explicitly.
```

When a concise replacement makes the fix obvious, include it directly:

```text
Suggested rule: Repeating the same request returns the existing result and does not create another record.
```

Do not rewrite large sections unless the user asks. Do not bury findings under a long summary.

If there are no actionable findings, say:

```text
No actionable findings. The spec is concise and sufficiently defines user behavior, business rules, and expected outcomes.
```
