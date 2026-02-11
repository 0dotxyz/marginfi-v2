## Build Notes

- Can only build the program or check if it's building using `anchor build -p marginfi -- --no-default-features --features custom-heap`

## Test Notes

- When running tests, use the @agent-anchor-test-runner agent with the specific script name from Anchor.toml
- Available test scripts: `basic-tests`, `kamino-tests`, `drift-tests`, `solend-tests`, `juplend-tests`, `staked-tests`, `all-tests`, etc.
- Output is saved to `test_output.txt` in the project directory for inspection

## TypeScript Error Checking

- To check TypeScript errors, use the MCP IDE diagnostics tool:
  - For a specific file: `mcp__ide__getDiagnostics` with `uri` parameter (e.g., `file:///root/projects/kamino-integration/tests/utils/types.ts`)
  - For all open files: `mcp__ide__getDiagnostics` without parameters
- Do NOT use `npx tsc --noEmit` as it's not configured correctly for this project
