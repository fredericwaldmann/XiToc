# M1.11 Firefox WASM observations

Captured 2026-11-11 from the local release package at 1280×900. These are actual
browser screenshots, separate from synthetic controller tests and native startup.

- `group-selected.png`: three rectangles after Ctrl+A / Ctrl+G.
- `group-context.png`: moved/resized group; Ungroup enabled, Group disabled.
- `group-locked.png`: attempted drag leaves locked geometry unchanged; Unlock target.
- `group-edit-scope.png`: one double-click enters the rotated duplicate's group.
- `group-text-exit.png`: child text survives text editing and Escape back to root.

The full physical diagram/zoom/theme/platform matrix was not performed; see
[TEST_PLAN](../../TEST_PLAN.md#m111--organization-verification-2026-11-11).
Canvas content remains in-memory; screenshots do not demonstrate PDF persistence.
