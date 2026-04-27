# Cross-cutting: Type System

## Scope
Applies to the type-checker, the executor (for coercion), and every family file (for type signatures).

## Rules

**R-TYPE-01 — The type vocabulary.** The system understands these types: Integer, Float, Boolean, Text, Date, DateTime, DateTimeZone, Duration, Time, Currency, Binary, Null, List-of-T (homogeneous, parameterised by an element type T), Function-of-T (parameterised by a return type T), and a Record type whose fields each carry their own type. There is also a top type (called Any, used for type variables) and a bottom type (called None, used for empty inputs whose element type cannot be inferred).

**R-TYPE-02 — Type-name mapping.** Each scalar type has a textual M-language name: Integer maps to *Int64.Type*, Float to *Number.Type*, Boolean to *Logical.Type*, Text to *Text.Type*, Date to *Date.Type*, DateTime to *DateTime.Type*, Time to *Time.Type*, Duration to *Duration.Type*, DateTimeZone to *DateTimeZone.Type*, Currency to *Currency.Type*, Binary to *Binary.Type*. The mapping is bijective except for Float, which is also reachable from the synonym *Double.Type*.

**R-TYPE-03 — Numeric predicate.** Integer, Float, and Currency are numeric. Every other scalar type is non-numeric.

**R-TYPE-04 — Comparable predicate.** Two types are comparable when they are equal, when both are numeric, when both are textual, when both are temporal in the same family (Date with Date, DateTime with DateTime, Time with Time, Duration with Duration), or when one is Null and the other is anything.

**R-TYPE-05 — Column-type inference.** Given the raw text values of a column, the system picks the most specific type all of them satisfy: if every value parses as an integer, the column is Integer; otherwise if every value parses as a finite floating-point number, the column is Float; otherwise if every value is the literal text *true* or *false*, the column is Boolean; otherwise the column is Text. An empty column defaults to Text. The literal text *null* (case-insensitive) is treated as a missing cell and ignored for inference; if every cell is missing, the type defaults to Null.

**R-TYPE-06 — Coercion of cell text.** Given a column's inferred type and one of its raw text values, coercion produces a runtime value: Integer parses to a 64-bit integer; Float parses to a 64-bit float; Boolean parses *true* and *false* case-insensitively; Text passes the raw text through. A failed parse at runtime is a type-mismatch execute-stage error (R-ERR-EXEC-TYPE).

**R-TYPE-07 — Type variables and unification.** The function catalogue's type signatures may contain type variables (single uppercase letters by convention, such as T). When checking a call site, the supplied argument types are unified against the signature's parameter types: a type variable matches any concrete type and binds to it for the rest of the unification. Once bound, the same variable used elsewhere in the signature stands for the same concrete type. Conflicts are R-ERR-TYPE-UNIFY.

**R-TYPE-08 — Function types.** A Function type carries its return type. Argument types of a function value are not currently tracked; a Function-of-T value can be applied with any argument shape, and only the return type matters for unification at the call site.

**R-TYPE-09 — List types.** A List-of-T is homogeneous: every element is of the inner type T. Mixing types in a list literal produces a List-of-Any.

**R-TYPE-10 — Record types.** A Record type is a finite ordered map from field name to field type. Two record types unify when their field-name sets are equal and their field-type pairs all unify.

**R-TYPE-11 — Optional and nullable parameters.** A signature may mark a parameter optional (the parameter may be omitted) or nullable (the parameter may receive Null in addition to its declared type). These are signature-level concepts; they do not appear in the type vocabulary itself.

**R-TYPE-12 — Type annotation flow.** The type-checker writes the inferred type onto every expression node and the output schema onto every step record. No later stage re-derives types; every later stage trusts these annotations (Source-of-Truth invariant R-INV-10).

## Examples

A column whose three cells are *1*, *2*, *3* is inferred Integer (R-TYPE-05). A column whose cells are *1*, *2.5*, *3* is inferred Float. A column whose cells are *true*, *false*, *true* is inferred Boolean. A column whose cells are *1*, *cat*, *3* is inferred Text.

A signature *List-of-T to T* unified against a call where the argument has type *List-of-Integer* binds T to Integer; the return type becomes Integer (R-TYPE-07).

## Test coverage

Pointers will live under `conformance/cross_cutting/type_system/`.

## Open questions

- R-TYPE-08: tracking parameter types of function values would tighten lambda-body checking; tracked in PROJECT_REPORT.
- R-TYPE-05: detection of Date and DateTime columns from raw text is not yet implemented; the column falls back to Text.

