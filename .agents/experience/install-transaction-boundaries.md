# Install Transaction Boundaries

Reusable rules for managed-file installation:

- Keep the next manifest state separate from files actually mutated during the
  current transaction. No-op writes may refresh manifest metadata, but must
  never enter failure rollback.
- Capture each mutated path exactly once at transaction start. Repeated writes,
  removes, or file/symlink transitions update the expected final state while
  retaining the original rollback snapshot.
- Pre-commit verification and failure rollback operate only on current
  transaction mutations. Full manifest verification remains a separate
  diagnostic for already-installed state.
- Test regular files, symlinks, removals, mixed repeated operations, and a real
  service-activation failure path. Include a credential-shaped fixture, but
  never use or serialize a real token.
