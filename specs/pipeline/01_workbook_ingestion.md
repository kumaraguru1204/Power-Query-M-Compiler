# Pipeline stage 1 — Workbook ingestion

**Input.** The workbook payload as a single string in the standard data-interchange format (JSON).

**What it does.**

1. Deserialise the payload into a three-field record: source name (string), sheet name (string), rows (list of lists of strings). Any structural deviation aborts the pipeline with the Json error category.
2. Reject if rows is empty (no header row available). Reject if every row's length is not equal to the header row's length — except that a row with *more* cells than the header is silently truncated to the header length (matching spreadsheet importer behaviour).
3. Split: the first row is the column-header list; subsequent rows are the data rows.
4. Build columns: one column per header, each holding the corresponding raw text value from every data row, with placeholder type Text.
5. Run the column-type-inference routine (R-TYPE-05) per column; replace the placeholder type with the inferred type.

**Output.** A typed table value carrying source name, sheet name, and an ordered list of typed columns. Each column carries name, inferred type, and raw text values.

**Failure modes.** Json error category. Sub-cases:
- Top-level value is not an object.
- Required field missing (source / sheet / rows).
- A row is not an array of strings.

**Storage shape.** A workbook record with three fields. A column record with three fields (name, inferred type, raw text values). Cells are not converted to typed values yet — coercion happens on demand at the executor.

