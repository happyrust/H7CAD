# H7CAD DXF Integration Development Plan

## Goal
Make H7CAD's DXF path easier to trust by closing the most visible gaps in the native-first migration: diagnostics, render coverage, and round-trip verification.

## Phases

1. [complete] Add DXF open diagnostics for partially preserved content.
2. [complete] Expand native render coverage or document fallback behavior for non-native-rendered DXF entities.
3. [complete] Add targeted round-trip tests around high-risk DXF entities.
4. [complete] Re-run workspace verification and summarize remaining gaps.

## Decisions

- Start with diagnostics because it is low risk, visible to users, and does not alter geometry.
- Preserve the native-first save/open path; do not remove the acadrust compatibility projection during this pass.
- Avoid committing or pushing until explicitly requested.
- Follow-up execution added an io-level `.dxf` open-path test so diagnostics are verified through the real loader, not only the helper.

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
