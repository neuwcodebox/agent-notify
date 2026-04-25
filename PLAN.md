# PLAN.md

## Goal

Build `agent-notify`, a Rust CLI that lets AI agents and automation scripts send notifications through configured channels.

Detailed behavior belongs in `SPEC.md`. This file only tracks the high-level implementation sequence.

## Phase 1: Project Skeleton

- Create the Rust workspace.
- Split the project into CLI and core crates.
- Set up basic command parsing.
- Add initial documentation files.

## Phase 2: Core Model and Configuration

- Define the common notification model.
- Load and validate TOML configuration.
- Resolve channel configuration.
- Add shared error handling and JSON output structure.

## Phase 3: Local Test Provider

- Implement the `file-log` provider first.
- Use it to validate message construction, attachments, and CLI behavior without external services.

## Phase 4: Webhook Provider

- Implement the standard webhook provider.
- Support the project-defined webhook protocol.
- Cover both message-only and attachment cases.

## Phase 5: Push and Chat Providers

- Implement `ntfy`.
- Implement Discord webhook support.
- Implement Telegram support.
- Implement Discord bot support.

## Phase 6: CLI Completion

- Finish `send`, `channels`, `check`, and `test`.
- Ensure human-readable and JSON outputs are stable.
- Normalize error handling across providers.

## Phase 7: Agent Skill and User Documentation

- Add the notification Agent Skill.
- Write README usage examples.
- Provide sample config and environment files.
- Document the webhook protocol.

## Phase 8: Validation and Release Prep

- Add tests for core behavior and error cases.
- Run formatting, linting, and test checks.
- Review documentation against `SPEC.md`.
- Prepare an initial release build.
- Publish the initial crates.io packages.

Status: complete for the initial crates.io release.
