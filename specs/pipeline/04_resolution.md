# Pipeline stage 4 — Name resolution

**Input.** The program tree from parsing, plus the input table.

**What it does.** Walks every binding in source order. Maintains two pieces of state:

- **Scope.** The set of step names defined so far.
- **Step-schema map.** For each step name defined so far, the ordered list of column names that step's output table is currently known to contain.

For each binding, the resolver:

1. Records the step's name into the scope. A duplicate is E205 (DuplicateStepName).
2. Walks the binding's contents and validates every reference:
   - Step references are checked against the scope. Unknown is E301 (UnknownStep), with a "did you mean?" suggestion based on Damerau-Levenshtein edit distance (threshold of two).
   - Column references (bracket access in row context, bare identifiers used in row context) are checked against the appropriate step-schema-map entry. Unknown is E302 (UnknownColumn), with a similar suggestion.
3. Computes the step's output schema:
   - Workbook initialiser: the input table's columns.
   - Sheet navigation: same as input.
   - Generic function call: invoke the function's schema-transform hook with the input schema and the call arguments (per the catalogue); if the hook is absent, propagate the input schema unchanged.
   - Value binding: empty schema (the binding is not table-shaped).
4. Records the computed schema into the step-schema map under the step's name.

**Output.** The same program tree, unchanged in shape, plus a fully populated step-schema map. The map is recomputed by the type-checker so it does not need to flow further; the resolver's purpose is fail-fast on names with precise suggestions.

**Failure modes.** Diagnostics category, codes E205 (carried over from parsing if duplicates appear), E301, E302. All diagnostics from one walk are returned together; the pipeline halts only after the resolver completes.

**Storage shape.** A scope set (just step names). A step-schema map keyed by step name, valued by ordered list of column names.

