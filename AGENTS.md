# AGENTS.md

## Source of Truth

Check `SPEC.md` before changing product behavior.

This file is only for coding guardrails. Do not treat it as a product specification.

## Scope Control

Do not add or remove features unless the task explicitly asks for it or `SPEC.md` already requires it.

Do not casually introduce:

- new notification providers
- new config formats
- background services
- schedulers
- daemon behavior
- secret scanning or attachment-blocking policies
- broad security policy layers

## Public Contract

The CLI interface and JSON output are automation-facing contracts.

Do not rename commands, flags, config keys, JSON fields, or error shapes without updating `SPEC.md`.

## Secrets

Do not add CLI flags that accept secrets directly.

Do not print resolved secrets in logs, errors, test snapshots, or debug output.

## Provider Boundaries

Keep provider-specific API details isolated from shared code.

Do not let one provider’s quirks reshape the common notification model unless the spec explicitly calls for it.

## Documentation Roles

- `SPEC.md`: product behavior and implementation requirements
- `PLAN.md`: implementation sequence
- `README.md`: user-facing usage
- `AGENTS.md`: coding guardrails only

## Skill Documentation

Keep skill docs brief and operational. Do not duplicate detailed JSON schemas, output contracts, or implementation behavior from `SPEC.md`; add only the minimal command guidance and examples an agent needs to use the feature.
