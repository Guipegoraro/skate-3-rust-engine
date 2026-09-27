# Domain docs

Single-context repo: one glossary and one ADR folder at the root.

- `CONTEXT.md`: the glossary (skating terms, engine terms, board terms). Read it before exploring the codebase for a card.
- `docs/adr/NNNN-<slug>.md`: decisions with a load-bearing reason (why a system is built this way, what was rejected). Read the ADRs touching the area you are about to change.
- `docs/<topic>.md`: per-feature implementation notes (how a system works today). These are notes, not decisions; an ADR records the *why*.

Both files are created lazily by `/domain-modeling` (reached through `/grill-with-docs` and `/improve-codebase-architecture`) the first time a term or decision is actually resolved. When they are missing, proceed silently.

Use the glossary's terms in card titles, spec sections, test names, type and module names. A concept you need that the glossary lacks is a signal: either the project has no word for it yet (note it for `/domain-modeling`) or you are inventing a synonym for one it has.

Output that contradicts an ADR says so explicitly: *"Contradicts ADR-0003 (…), worth reopening because …"*.
