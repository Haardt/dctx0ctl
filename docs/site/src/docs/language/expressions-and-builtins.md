# Expressions and built-ins

Expressions are typed, pure and target-neutral. They appear in refinements, invariants, preconditions, postconditions and lifecycle guards. The catalog is closed: no specification can define an expression evaluator. Explicitly pure module-owned surface operations may be called from invariants through the bound application; this is distinct from adding arbitrary built-ins.

## Operator precedence

From strongest to weakest:

```text
not  unary -
* / %
+ -
..
< <= > >= in
== != is
and
or
implies
```

Use parentheses when business meaning would otherwise be difficult to scan.

## `not`

Boolean negation. Its operand must be `Boolean`.

## Unary `-`

Numeric negation for `Int`, `Decimal` or `Float`.

## Arithmetic operators

`*`, `/`, `%`, `+` and `-` require numeric operands. Mixed numeric expressions promote to `Float`, then `Decimal`, then `Int` according to the participating types.

Do not use `/` when either operand is `Decimal`; the compiler requires `divide(left, right, scale, roundingMode)` so scale and rounding are explicit.

## `..`

Constructs an inclusive range such as `1..10`. In a `range` or `length` refinement it separates bounds; in an expression it creates a typed range value.

## Comparisons and equality

`<`, `<=`, `>`, `>=`, `==` and `!=` compare compatible typed values. `is` performs variant identity checks such as `failure is WorkshopNotFound`. Enum variants resolve as declarations, not strings.

## `in`

Membership or binder relation. In `value in collection` it produces a Boolean. In `item in items` inside `count`, `all` or `any`, it introduces the binder variable.

## `and`, `or`, `implies`

Boolean composition operators. `implies` is right-associative and is especially useful for conditional postconditions:

```dctx0
ensures success implies updates Workshop(id)
```

An implication is true when its premise is false; use it to separate success and failure obligations without executing both branches.

## Literals and construction

- Strings: `"text"`
- Booleans: `true`, `false`
- Integers, decimals and scientific numbers
- Lists: `[a, b, c]`
- Records: `Customer { id = customerId status = Active }`
- Fields: `result.id`
- Options: `Some(value)`, `None` (and accepted `None()` expression form)
- Payload-free variants: `Active`

Record construction uses `=` for field values; equality expressions use `==`.

## Collection operators

| Function | Result and purpose |
| --- | --- |
| `count(values)` | `Int` collection size |
| `count(item in values where predicate)` | Number of matching elements |
| `sum(values)` | Numeric sum |
| `sum(item.field for item in values)` | Project then sum |
| `all(item in values where predicate)` | Whether every element matches |
| `any(item in values where predicate)` | Whether any element matches |
| `contains(values, expected)` | Membership test |
| `distinct(values)` | List with duplicates removed |
| `min(values)` | Minimum element |
| `max(values)` | Maximum element |

Contextual `where` supplies a predicate for `count`, `all` or `any`. Contextual `for` in the `sum` projection introduces the source binder: `sum(item.amount for item in values)`.

## Option built-ins

- `Some(value)` creates `Option<T>` containing a value.
- `None`/`None()` creates an empty option.

## Text and shape built-ins

- `lowercase(text)` and `uppercase(text)` normalize case.
- `trim(text)` removes surrounding whitespace.
- `length(value)` returns the size of text or a collection.
- `startsWith(text, prefix)` and `endsWith(text, suffix)` test boundaries.
- `reverse(list)` returns the same list type in reverse order.

## Selection and transformations

| Built-in | Meaning |
| --- | --- |
| `filter(item in values where predicate)` | Keep matching elements |
| `map(expression for item in values)` | Project each element |
| `sortBy(expression for item in values)` | Sort by an ascending projected key |
| `sortDescBy(expression for item in values)` | Sort by a descending projected key |
| `slice(values, offset, limit)` | Select a bounded list window |
| `first(values)` | Optional first element |
| `single(values)` | Require a single element; invalid cardinality is explicit |
| `mean(values)` | Optional numeric average |
| `indexOf(values, item)` | Find the element position |
| `getOrElse(option, fallback)` | Select a value or default |
| `includesText(text, part)` | Text substring check |
| `ifElse(condition, thenValue, elseValue)` | Select between compatible typed expressions |

State-aware expressions can project declared `before.Entity` and `after.Entity` collections when actual observations provide those snapshots. A typed expression accepted by the compiler can still require context that a selected native adapter cannot supply.

## Decimal built-ins

```dctx0
divide(amount, divisor, 2, HalfEven)
round(amount, 2, HalfUp)
```

Rounding modes are `Down`, `Up`, `HalfUp` and `HalfEven`. They are compiler-known variants of `RoundingMode`.

## Contract-only calls

State predicates `exists`, `creates`, `updates`, `preserves`, and `removes` need operation state context; event predicates need observed events. They are not general invariant functions. Opaque `calls ... exactly ...` labels remain unmodeled, not executable resolved trace assertions.

## Contextual words and catalog values

- `returns` is valid for `given` stubs only; operation results always use `->`.
- Module dependencies use `requires`, executable interactions use `call`, and lifecycle discriminators use `using`. There is no `uses` language keyword.
- Record fields use `name: Type`.
- `Success`, `Failure`, `PASS`, `FAIL`, `UNKNOWN`, review/policy statuses and suite names are contextual values or catalog identifiers, not declaration keywords.
