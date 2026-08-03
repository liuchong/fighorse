# Code Connect Compatibility

fighorse native Code Connect support tracks the official `figma/code-connect`
CLI behavior observed at commit `6a6b50b1f71438768512e1b67475ba2bd555a018`
(CLI `1.5.1`).

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
