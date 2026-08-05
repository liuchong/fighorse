# fighorse Canvas Bridge Plugin

This plugin lets a local `fighorse` process write native Figma canvas nodes
through the Figma Plugin API.

## Pairing

1. Start the local bridge:

   ```bash
   fighorse canvas serve
   ```

2. Create a short-lived pairing code:

   ```bash
   fighorse canvas pair
   ```

3. In Figma Desktop, import `manifest.json` as a development plugin, run the
   plugin, enter the pairing code, and connect.

The plugin only connects to `127.0.0.1:9450`. It does not declare telemetry or
internet domains.

## Supported Editors

- Figma Design
- FigJam
- Figma Slides

All editors use the same protocol. Unsupported editor-specific operations are
rejected before changing the canvas.

