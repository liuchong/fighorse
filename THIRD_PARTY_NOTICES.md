# Third Party Notices

## Figma Code Connect

fighorse's native Code Connect compatibility layer is based on the public
behavior and source code of the official Figma Code Connect CLI.

- Upstream repository: https://github.com/figma/code-connect
- Compatibility baseline: `6a6b50b1f71438768512e1b67475ba2bd555a018`
- Upstream CLI version: `1.5.1`
- License: MIT

The upstream project is Copyright (c) 2024 Figma.

The MIT License permits use, copy, modification, merging, publishing,
distribution, sublicensing, and sale of copies of the software, provided that
the copyright notice and permission notice are included in all copies or
substantial portions of the software.

fighorse is not an official Figma product. Compatibility with observed Figma
Code Connect service endpoints is best-effort and may break if Figma changes
those endpoints.

## cursor-talk-to-figma-mcp

fighorse's local canvas bridge uses the general plugin-bridge pattern from the
MIT-licensed `grab/cursor-talk-to-figma-mcp` project as a design reference, but
the runtime here is implemented in Rust and does not include the upstream Bun,
Node.js, analytics, or unauthenticated channel protocol.

- Upstream repository: https://github.com/grab/cursor-talk-to-figma-mcp
- Reference commit: `ddd90f3a6d454ea0b2fc29f1b084f50fd062b880`
- License: MIT
