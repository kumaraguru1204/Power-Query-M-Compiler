# Interface — CLI

## Scope
The command-line entry point.

## Invocation

The CLI binary accepts the following arguments:

- a positional argument naming the formula file (required);
- a flag *--json <path>* naming the workbook payload file (optional; default payload used if absent);
- a flag *--debug* enabling token and AST output.

## Behaviour

1. Read the formula file's text into a string.
2. Read the JSON payload file's text into a string if provided; else use the default payload.
3. Call the shared compile function (`SOURCE_OF_TRUTH.md` §7.4) with the two strings and the debug flag.
4. On success, print to stdout, in this order:
   - the result table preview,
   - the cleaned formula,
   - the generated SQL,
   - the warnings (one per line, prefixed with the warning code),
   - the token list and AST dump when *--debug* was set.
5. On failure, print to stderr:
   - the rendered diagnostics (per `cross_cutting/13_error_model.md` §Rendering).

## Exit codes

- 0 — engine returned success.
- 1 — engine returned a Json, Lex, Parse, Diagnostics, or Execute error.
- 2 — argument-parsing error (missing required argument, file not found, etc.).

## Conformance
Pointers to fixtures will live under `conformance/interfaces/cli/`.

