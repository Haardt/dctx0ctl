const declarationKeywords = [
  "workspace", "domain", "scalar", "value", "type", "failure", "entity", "message", "event",
  "module", "surface", "port", "external", "operation", "decision", "example", "invariant",
  "lifecycle", "proof", "test", "review", "policy", "fragment"
]

const relationKeywords = [
  "owns", "requires", "effects", "read", "write", "call", "emit", "observe", "intent", "choose",
  "because", "avoid", "constraint", "given", "when", "expect", "fails", "emits", "forall", "inspect",
  "criteria", "question", "require", "range", "length", "format", "where", "key",
  "ensures", "state", "transition", "assert", "returns", "from", "to", "initial", "terminal",
  "exposes", "requestReply", "notification", "stream", "stableId", "pure"
]

module.exports = grammar({
  name: "dctx0",

  extras: ($) => [/\s/, $.comment],

  word: ($) => $.identifier,

  rules: {
    source_file: ($) => repeat($._item),

    _item: ($) => choice(
      $.declaration,
      $.declaration_keyword,
      $.keyword,
      $.builtin_type,
      $.identifier,
      $.string,
      $.triple_string,
      $.number,
      $.operator,
      $.block,
      $.list,
      $.arguments,
      /[:,;]/
    ),

    declaration: ($) => prec(1, seq(
      field("kind", $.declaration_keyword),
      field("name", $.identifier)
    )),

    block: ($) => seq("{", repeat($._item), "}"),
    list: ($) => seq("[", repeat($._item), "]"),
    arguments: ($) => seq("(", repeat($._item), ")"),

    declaration_keyword: () => choice(...declarationKeywords),
    keyword: () => choice(...relationKeywords),
    builtin_type: () => choice(
      "String", "Boolean", "Int", "Float", "Decimal", "UUID", "Timestamp", "Duration", "Bytes",
      "Unit", "Option", "List", "Some", "None", "Dctx0Option", "Dctx0Result"
    ),
    identifier: () => /[A-Za-z_][A-Za-z0-9_.-]*/,
    string: () => token(seq('"', repeat(choice(/[^"\\\n]/, /\\./)), '"')),
    triple_string: () => token(seq('"""', repeat(choice(/[^\"]/, /"[^\"]/, /""[^\"]/)), '"""')),
    number: () => /[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/,
    operator: () => choice(".", "->", "..", "!=", "<=", ">=", "==", "=", "!", "|", "<", ">", "+", "-", "*", "/", "%"),
    comment: () => token(choice(seq("//", /[^\n]*/), seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/")))
  }
})
