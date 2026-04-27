# Interface — Playground

## Scope
The browser-based playground served at the root path of the web entry point.

## Layout

A single HTML page containing:
- a left pane: a code editor where the user types the M formula;
- an upper-right pane: a JSON editor where the user provides the workbook payload (with a default already filled in);
- a lower-right pane: the result area, which switches between three tabs — Result Table, Formatted Formula, Generated SQL — plus a hidden Debug tab visible only when debug mode is on;
- a top bar: a Run button and a Debug toggle.

## Behaviour

1. On Run, the page packages the editors' contents into a JSON request body matching the POST /api/compile (or /api/compile/debug if Debug toggle is on) shape.
2. Sends the request to the appropriate route.
3. On success:
   - Result Table tab shows the *result* field rendered as a table.
   - Formatted Formula tab shows the *formatted* field with simple syntax highlighting.
   - Generated SQL tab shows the *sql* field.
   - Warnings, if any, appear as a small banner above the tabs.
4. On failure:
   - The result area shows the first error's *message* prominently, with the *source_line* underneath and an underline at *(column, length)*.
   - Subsequent errors appear in a scrollable list.
   - The code editor highlights the offending region of the formula.

## Cross-cutting

- The page degrades gracefully if the server is unreachable: the Run button shows a transient error toast.
- All API calls are POST to a relative path (no hard-coded host); the page works against any host serving it.
- The default workbook payload pre-populated in the JSON editor is the same default used by the shared compile function (R-INV-12).

## Conformance
Pointers to fixtures will live under `conformance/interfaces/playground/`. (Playground fixtures are end-to-end browser scenarios; tracked separately.)

