# Stub specs for partially-implemented or deferred functions

The functions below are catalogued in the function registry but have either no implementation, a partial implementation, or no clear family slot today. Each is acknowledged here so a reader who searches for it lands on a definitive note rather than a missing file.

A function listed in this stub file should be promoted to its own file as soon as its behaviour stabilises enough to spec.

---

## Table.ExpandTableColumn

Expands a column whose cells are tables (a nested-table column) into multiple rows by joining each row of the nested table to the parent row. Schema-widening; complex per-column-name argument list. **Status: not yet implemented; W002 in SQL.**

## Table.GetRelationships

Returns a table describing inter-table relationships (foreign keys). **Status: not implemented; the catalogue entry exists for parser parity only.**

## Table.TransformRows

Applies a row-context lambda to each row, returning a list of the lambda's return values. Belongs near F05 list higher-order but works on a Table input. **Status: partial; W002 in SQL.**

## List.Accumulate

Reduces a list to a single value by repeatedly applying a two-argument lambda (accumulator, element). Three required arguments: list, seed, combiner. **Status: partial; no SQL lowering.**

## List.Alternate

Returns elements of a list at alternating positions, defined by a count-skip-offset pattern. Four-argument form. **Status: partial.**

## List.Buffer

Forces a list to be fully materialised. **Status: passthrough — already eager (R-LIST-09); the function is a no-op semantically.**

## List.InsertRange

Inserts a sub-list at a given offset in a list. **Status: partial; no SQL lowering.**

## List.MaxN / List.MinN

Top-N or bottom-N elements of a list with comparison rules. **Status: partial.**

## List.Percentile

Returns a percentile of a numeric list. **Status: partial; no SQL lowering for arbitrary percentiles.**

## List.RemoveRange / List.ReplaceMatchingItems / List.ReplaceRange / List.ReplaceValue

Variants on cell or range replacement in a list. **Status: partial across the set; tracked.**

## List.Sort

Sorts a list. Optional comparator argument. **Status: implemented for the default natural-order case; comparator form deferred.**

## List.Split

Partitions a list into a list of sub-lists of a given size. **Status: partial.**

## List.TransformMany

Like List.Transform but the lambda returns a list per element; the results are flattened. **Status: partial; no SQL lowering.**

## List.Zip

Zips a list of lists into a list of tuples (lists). **Status: partial.**

## Text.Combine

Joins a list of texts with a delimiter. **Status: partial; SQL lowering uses STRING_AGG where available.**

---

Each of these functions deserves its own file once the implementation stabilises. Until then, this stub serves as the deterministic landing place. **Do not** add new functions to this stub; create either a thin leaf or a full file per the promotion criteria in `00_conventions.md`.

