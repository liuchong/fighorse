# Code Connect Compatibility

fighorse native Code Connect support tracks the official `figma/code-connect`
CLI behavior observed at commit `d856baa9bf7907c430121139725a7a8181bac9a3`
(CLI `2.1.0`).

Reusable checks:

- Treat `/v1/code_connect`, `/v1/code_connect/preview_snippets`, and
  `/v1/code_connect/{file_key}/cli_data` as observed private protocol, not
  public REST coverage.
- Do not execute `.figma.ts` or `.figma.js` templates locally. fighorse parses,
  validates, and sends them to Figma for real preview/publish.
- Reject project-configured `apiUrl`; otherwise a repository can exfiltrate a
  Figma token.
- Publishing and unpublishing are remote writes. MCP must require both explicit
  Code Connect egress permission and Figma write mode.
- If Figma returns an unexpected status or response shape, report
  `protocol_incompatible` with the baseline commit and failed phase.
