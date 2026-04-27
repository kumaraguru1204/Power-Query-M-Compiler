# Cross-cutting: Invariants

## Scope
Applies to the entire system. This file restates `SOURCE_OF_TRUTH.md` §9 with rule identifiers and short rationale.

## Rules

**R-INV-01 — One pipeline.** Every entry point reaches the same shared compile function, which reaches the same nine-stage pipeline. There is no second pathway that bypasses any stage.

*Rationale.* Behaviour parity across console, CLI, and web is achievable only when every entry point shares one code path.

**R-INV-02 — Order is fixed.** Stages run in this order: workbook ingestion, lex, parse, resolve, type-check, format, execute, SQL emit, package. No stage is skipped, no stage runs out of order, no later stage fixes up what an earlier stage got wrong.

*Rationale.* Out-of-order execution would mean each stage's contract becomes context-dependent; reasoning collapses.

**R-INV-03 — Position markers are mandatory.** Every token, every node of the parsed tree, every diagnostic label carries a position marker (real or dummy). No silent discarding.

*Rationale.* Precise error messages depend on this. A missing position marker becomes a missing error.

**R-INV-04 — Diagnostics are typed records.** Errors that originate inside the pipeline are diagnostic records, not strings. Conversion to plain text happens only at the renderer or the web-layer flattening boundary.

*Rationale.* Records can be filtered, sorted, deduplicated, and post-processed; strings can only be appended.

**R-INV-05 — Function knowledge is data.** Adding or changing an M function means editing the function catalogue plus per-function entries in the executor and the SQL emitter — and nothing else. Bespoke parser branches per function are a design smell; the catalogue's argument-hint vocabulary should grow to absorb any new shape.

*Rationale.* Per-function code paths in the parser are how compilers fall apart at scale.

**R-INV-06 — The grammar sub-project has no upward dependencies.** It is the dictionary; the rest of the system reads from it. Nothing in the grammar sub-project may know about the executor, the SQL emitter, or the engine.

*Rationale.* The catalogue must be importable from any future tool (a documentation generator, an autocomplete server, a code formatter for an external editor) without dragging in execution.

**R-INV-07 — The diagnostics sub-project has no horizontal dependencies.** It depends on nothing else in the workspace. It is the alphabet for all errors.

*Rationale.* Every other sub-project must be free to construct diagnostics without circular imports.

**R-INV-08 — The runtime value vocabulary is private.** Only the executor reasons in runtime values. No other sub-project sees them.

*Rationale.* Static types travel through the public surface; runtime values are an implementation detail.

**R-INV-09 — Tables are immutable in transit.** Each step produces a fresh table. No step mutates its input table in place.

*Rationale.* Aliasing of mutable tables makes step-level memoisation impossible and debugging awful.

**R-INV-10 — Type annotations, once written, are trusted.** The type-checker is the only writer of inferred-type and output-schema slots. Every later stage reads them and trusts them; no later stage re-derives types.

*Rationale.* If two stages re-derive types they will eventually disagree.

**R-INV-11 — The formatter is a pure function of the parsed tree.** It does not consult the input table, the executor, or the SQL emitter.

*Rationale.* Formatting must be deterministic and side-effect-free.

**R-INV-12 — The default payload is for fallback only.** Production callers always supply a real payload. The default exists so that an empty request still returns a meaningful response for testing.

*Rationale.* Confusion arises when a developer forgets to supply data and silently receives the default.

**R-INV-13 — Behaviour is identical across entry points.** A bug visible only in the web entry point but not in the console entry point lies in the entry-point glue, not in the engine.

*Rationale.* This is a triangulation aid for diagnosing reported bugs.

## Test coverage

A test suite that exercises each invariant lives under `conformance/cross_cutting/invariants/`. Tracking the existence of every test file against this list is part of the conformance discipline.

## Open questions

None. The invariants list is intentionally small and stable.

