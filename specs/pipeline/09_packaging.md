# Pipeline stage 9 — Packaging

**Input.** All artefacts produced by stages 1 through 8.

**What it does.** Assembles a single output bundle carrying the seven artefacts listed in `SOURCE_OF_TRUTH.md` §2.2:

1. The input table.
2. The result table.
3. The cleaned formula text.
4. The generated SQL.
5. The parsed program (for debugging tools).
6. The token list (for debugging tools).
7. A success indication plus an optional list of warnings.

**Output.** The bundle.

**Failure modes.** None. By the time stage 9 runs, the pipeline has succeeded.

**Storage shape.** A record carrying the seven fields above.

