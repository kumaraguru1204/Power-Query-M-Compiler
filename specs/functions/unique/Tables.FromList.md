# Tables.FromList

## 1. Identity

- **Full name.** Tables.FromList.
- **Namespace.** Tables.
- **Family.** None (table-construction from a list).
- **Status.** Partial.

## 2. Argument shape

- **Argument 1.** A list (StepRefOrValue).
- **Argument 2 (optional).** A splitter lambda (typically *Splitter.SplitTextByDelimiter*).
- **Argument 3 (optional).** A list of column names.
- **Argument 4 (optional).** A default value for missing fields.
- **Argument 5 (optional).** An ExtraValues policy keyword.

## 3. Type signature

*(List-of-T [, splitter] [, List-of-Text] [, default] [, extra-values-policy]) → Table*.

## 4. Schema-transform rule

Output schema has one column per declared column name; element type is Text by default, refined by the splitter / coercion as the implementation matures.

## 5. Runtime semantics

For each element of the input list, apply the splitter (default: identity) to produce a tuple of values. Pad with the default if shorter than the column-name list; truncate or error per the ExtraValues policy if longer.

## 6. SQL lowering

Not yet implemented. Emits W002.

## 7. Reference behaviour

Aligns with official M.

## 8. Conformance fixtures

`conformance/functions/Tables.FromList/`.

## 9. Open questions and known gaps

- Most of the optional arguments are partially implemented; tracked in PROJECT_REPORT.

