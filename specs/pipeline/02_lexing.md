# Pipeline stage 2 — Lexing

**Input.** The formula text as a single string.

**What it does.** Implements the state machine in `cross_cutting/01_lexical_grammar.md`. Walks the input one character at a time, maintaining offset, line, and column counters. At each step:

1. Skip whitespace (R-LEX-01).
2. Inspect the next character and dispatch into one of: string-literal scanning, numeric-literal scanning, identifier-or-keyword scanning, quoted-identifier scanning, single-character punctuation, multi-character operator disambiguation, unknown-character error.
3. Build a token whose kind matches the lexed shape and whose position marker covers the lexed region.

After the input is exhausted, append an end-of-input token (R-LEX-15).

**Output.** A flat list of tokens. Each token: kind (one of a fixed enumeration covering literals, identifiers, keywords, comparison operators, arithmetic operators, the concatenation operator, the dot, the comma, the fat-arrow, the four bracket pairs, and the end-of-input marker), position marker.

**Failure modes.** Lex error category. Sub-cases mapped to E101 (UnterminatedString), E102 (UnexpectedCharacter), E103 (InvalidNumber), E104 (UnsupportedComment).

All lex diagnostics are collected (lexing does not stop on the first error); the pipeline halts only after the lex pass completes if any diagnostics were produced.

**Storage shape.** A list of token records.

