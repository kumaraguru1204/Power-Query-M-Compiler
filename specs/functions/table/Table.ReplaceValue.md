# Table.ReplaceValue

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Replaces cells matching a needle with a replacement, in the named columns. Five arguments: table, old value, new value, replacer-lambda (typically *Replacer.ReplaceValue* or *Replacer.ReplaceText*), column-name list.

**Schema rule.** Schema preserved.

**Edge cases.**
- The replacer lambda is a two-arg function the user supplies; today the catalogue treats it as a black-box equality-based replacer. Per-replacer dispatch is partial.
- Column unknown: E302.

**Conformance.** `conformance/functions/Table.ReplaceValue/`.

