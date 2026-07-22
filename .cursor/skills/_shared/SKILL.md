---
name: _shared
description: "Shared SDD references for installed skills. Not invokable."
disable-model-invocation: true
user-invocable: false
license: MIT
metadata:
  author: gentleman-programming
  version: "1.0"
---

## Purpose

This directory stores shared reference documents consumed by real SDD skills
(for example: `sdd-phase-common.md`, `persistence-contract.md`,
`workspace-rules.md`).

## Workspace rules

All project skills and delegated sub-agents MUST honor `workspace-rules.md`
(indexes always-apply `.cursor/rules/`, including Explore/Library model routing).

## Not Invokable

`_shared` is a support package only. Do not invoke it as a skill.
