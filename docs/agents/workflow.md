# Card workflow

Every code change belongs to a card, and every card moves through the same steps. The card's state decides the next skill; this table is the single source of truth for that decision (the `/next` skill and the session-start hook read the board and apply it).

| Card state | Next step |
|---|---|
| Picked from `idea`/`new`, `## Spec` absent, decisions still open | `/grill-with-docs`, sized to the card: a one-line fix needs two questions, a new system needs the full tree. Decisions with a load-bearing reason land in an ADR, new terms in `CONTEXT.md`. |
| Grilled, `## Spec` absent | `/to-spec`: write `## Spec` into the card (implementation decisions, the seams to test at, out of scope). Confirm the seams with the user before writing. |
| `## Spec` larger than one session | `/to-tickets`: one child card per tracer-bullet slice, `depends_on` for the blockers; work the frontier. |
| In `doing`, seams agreed | `/implement`: `/tdd` at those seams (`cargo test` crates, or a GameTest scenario for in-game behaviour), `cargo check` per iteration. |
| Code done, uncommitted | `/code-review` since the card's first commit: Standards axis = `CLAUDE.md` code rules, Spec axis = the card's acceptance criteria. Fix findings, add the README bullet, commit naming the card. |
| Verified by build/tests/screenshots | Move to `finished`; leftover scope becomes a new card. `testing` only for feel or controller behaviour the user must try. |
| Waiting on the user | Move to `voce` with a `> @claude … PARA VOCE:` note listing exactly what to try or decide. |
| Bug reported (chat, card note, log) | `/diagnosing-bugs`: first a loop that goes red on the bug (a test or GameTest scenario), then the fix, then the regression test stays. |
| Upstream merge stops on conflicts | `/resolving-merge-conflicts`. |
| Session ending mid-card | `/handoff` into the card's `## Progress`. |
| Every few days, or a hot spot keeps hurting | `/improve-codebase-architecture` on the area `git log` says changes most. |

## Announce the step

At the start of work on a card, and again whenever the state changes, say in one line: card id, its state, the next step from this table. A skipped step is announced as a skip with its reason ("SK-070 is a typo fix: skipping grilling and spec"). The user can then say "no, grill it".

## Commit gate

A hook refuses `git commit` when a code change lacks its `README.md` bullet or the message lacks a card id (`SK-nnn`); commits touching only the board, `docs/` and agent config pass. The refusal names the missing piece: add it and commit again.
