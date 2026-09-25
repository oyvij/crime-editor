//! F14 — colouring code by token type.
//!
//! What this module decides is which *kind* a piece of text is. Colour is a
//! theme's business and lives at the edge, which is why the scenarios assert
//! kinds and never colours.

use std::sync::OnceLock;
use syntect::easy::ScopeRegionIterator;
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Keyword,
    Operator,
    String,
    Comment,
    Number,
    Constant,
    Function,
    Type,
    Property,
    Attribute,
    Punctuation,
    Markup,
    Invalid,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub kind: Kind,
}

/// `bat`'s 220-grammar set, not syntect's default 75 — which languages and why
/// is the `two-face` row in `docs/stack.md`. Two things bind the call: the
/// no-newlines variant is the one the line-at-a-time parse below needs, and the
/// set stays behind `OnceLock` because loading it is the whole cost.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(two_face::syntax::extra_no_newlines)
}

/// Tokens for `source`, one entry per line, with the language taken from
/// `name` — a file name's extension, or a language name given directly (a
/// fenced code block's info string, which has no filename to invent). A
/// language unknown to syntect comes back as one plain token per line, exactly
/// as an unknown extension already does.
///
/// The two lookups are why *no extension* does not mean *no language*: `LICENSE`
/// renders plain, but `Dockerfile` and `Makefile` are named by the grammar that
/// wants them and colour on the name alone.
///
/// Grouped by line rather than flat, because *what colour is line N* is a
/// question two surfaces ask — the editor's rows and Review's diff — and a flat
/// stream makes each of them count newlines for itself. The parse is still one
/// pass over the whole source, which is the only way a token that spans lines
/// (a block comment, a multi-line string) colours the lines under it: highlight
/// a line on its own and it restarts, and a row beginning inside a string reads
/// as code.
pub fn highlight(name: &str, source: &str) -> Vec<Vec<Token>> {
    let set = syntaxes();
    let syntax = name
        .rsplit_once('.')
        .and_then(|(_, extension)| set.find_syntax_by_extension(extension))
        .or_else(|| set.find_syntax_by_token(name));
    let Some(syntax) = syntax else {
        return plain(source);
    };

    let mut parse = ParseState::new(syntax);
    // Both the parse and the scope stack outlive the line: `parse_line` reports
    // only the scopes a line *changes*, so a stack rebuilt per line starts
    // empty and knows nothing about the block comment or multi-line string the
    // line is inside. That is what made a comment's second line read as code —
    // a whole-file parse that was still, in effect, one line at a time.
    let mut stack = ScopeStack::new();
    let mut lines = Vec::new();
    for line in source.split('\n') {
        let mut tokens = Vec::new();
        let Ok(ops) = parse.parse_line(line, set) else {
            push(&mut tokens, line, Kind::Plain);
            lines.push(tokens);
            continue;
        };
        for (text, op) in ScopeRegionIterator::new(&ops, line) {
            if stack.apply(op).is_err() {
                continue;
            }
            if !text.is_empty() {
                push(&mut tokens, text, classify(&stack));
            }
        }
        lines.push(tokens);
    }
    lines
}

pub fn plain(source: &str) -> Vec<Vec<Token>> {
    source.split('\n').map(plain_line).collect()
}

fn plain_line(line: &str) -> Vec<Token> {
    match line.is_empty() {
        true => Vec::new(),
        false => vec![Token {
            text: line.to_string(),
            kind: Kind::Plain,
        }],
    }
}

/// Tokens for `source` out of the tokens of an earlier text of the same file,
/// for as long as the parse of `source` itself is running off the main loop
/// (#101). Every line the edit left alone keeps its colours — matched from the
/// top and from the bottom, which is where an edit leaves lines alone — and the
/// lines between are plain. The tokens carry the text they colour, so the old
/// ones drawn as they were would show the file as it was before the key.
///
/// Owned, and cut rather than copied: it runs on the main loop once per edit,
/// and a copy of every token of a big file would be the cost being moved off
/// it.
pub fn carried(mut tokens: Vec<Vec<Token>>, source: &str) -> Vec<Vec<Token>> {
    let lines: Vec<&str> = source.split('\n').collect();
    let holds = |tokens: &[Token], line: &str| {
        let mut rest = line;
        tokens
            .iter()
            .all(|token| match rest.strip_prefix(token.text.as_str()) {
                Some(after) => {
                    rest = after;
                    true
                }
                None => false,
            })
            && rest.is_empty()
    };
    let both = tokens.len().min(lines.len());
    let above = (0..both)
        .take_while(|&index| holds(&tokens[index], lines[index]))
        .count();
    let below = (1..=both - above)
        .take_while(|&back| holds(&tokens[tokens.len() - back], lines[lines.len() - back]))
        .count();
    let end = tokens.len() - below;
    tokens.splice(
        above..end,
        lines[above..lines.len() - below]
            .iter()
            .map(|line| plain_line(line)),
    );
    tokens
}

/// Adjacent text of the same kind is one token, so a quoted string arrives
/// whole rather than split around its quote marks.
fn push(tokens: &mut Vec<Token>, text: &str, kind: Kind) {
    match tokens.last_mut() {
        Some(last) if last.kind == kind => last.text.push_str(text),
        _ => tokens.push(Token {
            text: text.to_string(),
            kind,
        }),
    }
}

/// Sublime scopes, innermost first — but a delimiter belongs to what it
/// delimits (R14.6). A string's quotes are `punctuation.definition.string.begin`
/// *inside* the `string` scope and a comment's `//` is punctuation inside the
/// `comment` scope, so punctuation winning the innermost-first walk would split
/// `"world"` into three tokens. It is the weakest kind: taken only when nothing
/// else in the stack claims the text, and still beating plain — otherwise a `,`
/// wrapped in nothing but `meta.group` and `source` would come back gray.
fn classify(stack: &ScopeStack) -> Kind {
    let mut weakest = Kind::Plain;
    for scope in stack.scopes.iter().rev() {
        match scope_kind(&scope.build_string()) {
            Kind::Plain => {}
            Kind::Punctuation => weakest = Kind::Punctuation,
            claimed => return claimed,
        }
    }
    weakest
}

/// The convention's fifteen families, total by construction (R14.5). The second
/// segment is read only where a family is genuinely two things, which is the
/// whole of the per-grammar variation.
///
/// `meta` stays plain deliberately: it is a *structural* scope covering whole
/// regions — `meta.block`, `meta.function.parameters` — so colouring the family
/// would colour half the file. It is the arm that makes a grammar's precision
/// visible rather than one to work around: Sublime's default Rust grammar
/// scoped `Vec` as nothing but `meta.generic`, so it came back gray, and the
/// extended set names it `support.type` and it turns blue with no change here.
/// A gap in a grammar is fixed by the grammar.
///
/// The `keyword.operator` arm must stay above the general `keyword` one: every
/// grammar spells `=` and `+` inside the keyword family, so the broader arm
/// swallows them and every operator in every language renders as `if` does.
fn scope_kind(name: &str) -> Kind {
    let rest = name
        .split_once('.')
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    match name.split('.').next().unwrap_or_default() {
        "comment" => Kind::Comment,
        "string" => Kind::String,
        "punctuation" => Kind::Punctuation,
        "invalid" => Kind::Invalid,
        "markup" => Kind::Markup,
        "keyword" if rest.starts_with("operator") => Kind::Operator,
        "keyword" | "storage" => Kind::Keyword,
        "constant" if rest.starts_with("numeric") => Kind::Number,
        "constant" => Kind::Constant,
        "entity" if rest.starts_with("name.function") => Kind::Function,
        // A TOML or YAML mapping key is `entity.name.tag`, the same scope HTML
        // uses for a tag name, so a key is markup and not a property: one scope
        // cannot be two kinds without branching on the language.
        "entity"
            if rest.starts_with("name.tag")
                || rest.starts_with("name.table")
                || rest.starts_with("name.section") =>
        {
            Kind::Markup
        }
        "entity" if rest.starts_with("other.attribute-name") => Kind::Attribute,
        "entity" => Kind::Type,
        "support" if rest.starts_with("function") => Kind::Function,
        "support" if rest.starts_with("type") || rest.starts_with("class") => Kind::Type,
        "support" => Kind::Constant,
        "variable" if rest.starts_with("annotation") => Kind::Attribute,
        "variable"
            if rest.starts_with("parameter")
                || rest.starts_with("other.property")
                || rest.starts_with("object.property") =>
        {
            Kind::Property
        }
        "variable" => Kind::Plain,
        // `meta`, `source`, `text` and `embedded` — and any family a grammar
        // invents that nobody has named yet.
        _ => Kind::Plain,
    }
}

#[cfg(test)]
mod tests {
    use super::{carried, highlight, plain, scope_kind, Kind, Token};

    /// #101: what an edit is drawn in while its parse runs on a thread. The
    /// lines it left alone keep their colours, counted from the top and from
    /// the bottom, and only what it changed is plain until the parse lands.
    #[test]
    fn an_edit_keeps_the_colours_of_every_line_it_left_alone() {
        let before = highlight("main.rs", "fn a() {}\nlet x = 1;\nfn b() {}");
        let after = carried(
            before.clone(),
            "fn a() {}\nlet x = 12;\nlet y = 2;\nfn b() {}",
        );
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1..3], plain("let x = 12;\nlet y = 2;")[..]);
        assert_eq!(after[3], before[2]);
    }

    /// Whatever was carried, the text drawn is the text now: tokens carry the
    /// text they colour, so a line matched wrongly would draw what the file
    /// held before the key. Lines removed, lines repeated and an emptied file
    /// are where counting from both ends overlaps.
    #[test]
    fn carried_tokens_hold_the_text_as_it_is_now() {
        let text = |tokens: &[Vec<Token>]| {
            tokens
                .iter()
                .map(|line| {
                    line.iter()
                        .map(|token| token.text.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        for (before, after) in [
            ("a\nb\nc", "a\nc"),
            ("a\na\na", "a\na"),
            ("a\na", "a\na\na"),
            ("fn a() {}\n", ""),
            ("", "fn a() {}"),
            ("x", "x"),
        ] {
            assert_eq!(
                text(&carried(highlight("main.rs", before), after)),
                after,
                "{before:?} to {after:?}"
            );
        }
    }
    use syntect::easy::ScopeRegionIterator;
    use syntect::parsing::{ParseState, ScopeStack};

    fn kind_of(name: &str, source: &str, needle: &str) -> Kind {
        highlight(name, source)
            .into_iter()
            .flatten()
            .find(|token| token.text.trim() == needle)
            .unwrap_or_else(|| panic!("no token {needle:?} in {source:?}"))
            .kind
    }

    /// Every top-level scope family the grammar produces for `source`.
    fn families(name: &str, source: &str) -> Vec<String> {
        let set = super::syntaxes();
        let syntax = set
            .find_syntax_by_extension(name.rsplit_once('.').expect("an extension").1)
            .expect("a known language");
        let mut parse = ParseState::new(syntax);
        let mut seen = Vec::new();
        for line in source.split('\n') {
            let ops = parse.parse_line(line, set).expect("a parsed line");
            let mut stack = ScopeStack::new();
            for (_, op) in ScopeRegionIterator::new(&ops, line) {
                stack.apply(op).expect("a valid scope operation");
                for scope in &stack.scopes {
                    let family = scope
                        .build_string()
                        .split('.')
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    if !seen.contains(&family) {
                        seen.push(family);
                    }
                }
            }
        }
        seen
    }

    /// The grouping both the editor's rows and Review's diff index by. Three
    /// claims in one source, because they are the same claim: a line's tokens
    /// sit at its own position, a line with nothing on it holds none, and a
    /// comment opened on one line still colours the next — which is the whole
    /// reason the parse runs over the source rather than over each line.
    #[test]
    fn tokens_are_grouped_by_line() {
        let lines = highlight("a.rs", "let x = 1;\n\n/* note\n   still */");
        let text: Vec<String> = lines
            .iter()
            .map(|tokens| tokens.iter().map(|token| token.text.as_str()).collect())
            .collect();
        assert_eq!(text, ["let x = 1;", "", "/* note", "   still */"]);
        assert_eq!(lines[1], Vec::new());
        assert_eq!(lines[3][0].kind, Kind::Comment, "{:?}", lines[3]);
    }

    #[test]
    fn storage_words_count_as_keywords() {
        assert_eq!(kind_of("a.rs", "let x = 1;", "let"), Kind::Keyword);
        assert_eq!(kind_of("a.rs", "fn main() {}", "fn"), Kind::Keyword);
    }

    #[test]
    fn a_string_arrives_whole_including_its_quotes() {
        assert_eq!(kind_of("a.rs", "let s = \"hi\";", "\"hi\""), Kind::String);
    }

    #[test]
    fn numbers_are_not_keywords() {
        assert_eq!(kind_of("a.rs", "let x = 42;", "42"), Kind::Number);
    }

    #[test]
    fn a_language_name_works_as_well_as_a_file_name() {
        assert_eq!(kind_of("rust", "fn main() {}", "fn"), Kind::Keyword);
    }

    #[test]
    fn an_unknown_language_name_renders_as_plain() {
        let tokens = highlight("not-a-real-language", "fn main() {}");
        assert!(
            tokens
                .iter()
                .flatten()
                .all(|token| token.kind == Kind::Plain),
            "{tokens:?}"
        );
    }

    #[test]
    fn operators_are_not_keywords() {
        assert_eq!(kind_of("a.rs", "let x = 1 + 2;", "="), Kind::Operator);
        assert_eq!(kind_of("a.rs", "let x = 1 + 2;", "+"), Kind::Operator);
    }

    /// Rust spells every operator `keyword.operator`; JavaScript distinguishes
    /// `.assignment`, `.arithmetic` and `.comparison`. Both must land here, and
    /// the control keyword beside them must not move.
    #[test]
    fn assignment_arithmetic_and_comparison_operators_in_two_languages() {
        for (name, source) in [
            ("a.rs", "if a == b { c += 1; }"),
            ("a.js", "if (a === b) { c += 1; }"),
        ] {
            assert_eq!(kind_of(name, source, "if"), Kind::Keyword, "{name}");
            assert_eq!(kind_of(name, source, "+="), Kind::Operator, "{name}");
            let comparison = if name.ends_with(".rs") { "==" } else { "===" };
            assert_eq!(kind_of(name, source, comparison), Kind::Operator, "{name}");
        }
    }

    /// The convention's fifteen top-level families. Every one maps, so a
    /// language nobody anticipated is coloured on arrival (R14.5).
    const FAMILIES: [(&str, Kind); 15] = [
        ("comment", Kind::Comment),
        ("constant", Kind::Constant),
        ("embedded", Kind::Plain),
        ("entity", Kind::Type),
        ("invalid", Kind::Invalid),
        ("keyword", Kind::Keyword),
        ("markup", Kind::Markup),
        ("meta", Kind::Plain),
        ("punctuation", Kind::Punctuation),
        ("source", Kind::Plain),
        ("storage", Kind::Keyword),
        ("string", Kind::String),
        ("support", Kind::Constant),
        ("text", Kind::Plain),
        ("variable", Kind::Plain),
    ];

    #[test]
    fn every_convention_family_maps_to_a_kind() {
        for (family, kind) in FAMILIES {
            assert_eq!(scope_kind(family), kind, "{family}");
        }
    }

    /// The guard on a sixteenth family: parse a language per family and assert
    /// every top-level scope the grammars actually produce is one the table
    /// names. A family nobody handled would arrive here before it arrived on
    /// screen as gray.
    #[test]
    fn no_grammar_produces_a_family_the_table_does_not_name() {
        let corpus = [
            (
                "a.rs",
                "#[derive(Debug)]\nfn f(x: u8) -> Vec<String> { let s = \"hi\"; }",
            ),
            (
                "a.js",
                "// note\nconfig.debug = false;\nfunction f(a) { return a + 1; }",
            ),
            ("a.py", "def f(a, b): return None"),
            ("a.go", "x := 09"),
            ("a.java", "@Override public class A { }"),
            ("a.css", "a { color: red; }"),
            ("a.yaml", "key: value"),
            ("index.html", "<div class=\"a\">hi</div>"),
            ("README.md", "A **bold** word."),
        ];
        for (name, source) in corpus {
            for family in families(name, source) {
                assert!(
                    FAMILIES.iter().any(|(known, _)| *known == family),
                    "{name} produced the unhandled family {family:?}"
                );
            }
        }
    }

    #[test]
    fn a_function_call_is_a_function() {
        assert_eq!(
            kind_of("a.rs", "let n = compute(1);", "compute"),
            Kind::Function
        );
    }

    #[test]
    fn language_literals_are_constants_and_numbers_stay_numbers() {
        assert_eq!(kind_of("a.rs", "let ok = true;", "true"), Kind::Constant);
        assert_eq!(
            kind_of("a.js", "const ok = false;", "false"),
            Kind::Constant
        );
        assert_eq!(kind_of("a.py", "x = None", "None"), Kind::Constant);
        assert_eq!(kind_of("a.rs", "let x = 42;", "42"), Kind::Number);
    }

    /// R14.6: a delimiter belongs to what it delimits. The quotes are
    /// `punctuation.definition.string.begin` *inside* the string scope and the
    /// `//` is punctuation inside the comment, so a punctuation arm that won
    /// the innermost-first walk would split both.
    #[test]
    fn punctuation_is_the_weakest_kind() {
        let source = "let point: Pair = make(1, 2);";
        assert_eq!(kind_of("a.rs", source, ":"), Kind::Punctuation);
        assert_eq!(kind_of("a.rs", source, ","), Kind::Punctuation);
        assert_eq!(kind_of("a.rs", "let s = \"hi\";", "\"hi\""), Kind::String);
        assert_eq!(kind_of("a.rs", "// a note", "// a note"), Kind::Comment);
        assert_eq!(
            kind_of("README.md", "A **bold** word.", "**bold**"),
            Kind::Markup
        );
    }

    /// A macro, annotation or decorator is `variable.annotation` — measured,
    /// not the `entity.other.attribute-name` the convention suggests.
    #[test]
    fn an_annotation_is_an_attribute() {
        assert_eq!(
            kind_of("a.rs", "#[derive(Debug)]", "derive"),
            Kind::Attribute
        );
        assert_eq!(
            kind_of("a.java", "@Override public class A { }", "Override"),
            Kind::Attribute
        );
    }

    #[test]
    fn a_tag_is_markup_and_its_attribute_name_is_an_attribute() {
        let source = "<div class=\"a\">hi</div>";
        assert_eq!(kind_of("index.html", source, "div"), Kind::Markup);
        assert_eq!(kind_of("index.html", source, "class"), Kind::Attribute);
    }

    /// A mapping key is `entity.name.tag` — the same scope as an HTML tag —
    /// so it takes markup, not property. One scope cannot be two kinds
    /// without branching on the language.
    #[test]
    fn a_mapping_key_is_markup() {
        assert_eq!(kind_of("a.yaml", "key: value", "key"), Kind::Markup);
    }

    /// The languages the default 75-syntax set did not name (R14.7), each with
    /// one anchor whose kind is unambiguous. A row pins an *extension*, so two
    /// rows sharing a snippet — `.tsx` and `.jsx` — are two grammars, not a
    /// copied line. Resolution alone is not the claim:
    /// a language that stopped resolving comes back as a single plain token, so
    /// `kind_of` fails to find the anchor at all. The last two entries are
    /// language *names* rather than file names — a fenced code block has no
    /// filename to invent, and its info string must reach the same set.
    #[test]
    fn the_extended_set_colours_the_languages_the_default_set_missed() {
        let corpus = [
            (
                "a.ts",
                "const total: number = add(1, 2);",
                "const",
                Kind::Keyword,
            ),
            (
                "a.tsx",
                "const el = <Box name=\"a\" />;",
                "name",
                Kind::Attribute,
            ),
            (
                "a.jsx",
                "const el = <Box name=\"a\" />;",
                "name",
                Kind::Attribute,
            ),
            (
                "A.vue",
                "<template><div class=\"a\">hi</div></template>",
                "class",
                Kind::Attribute,
            ),
            (
                "A.svelte",
                "<script>let n = 1;</script>",
                "let",
                Kind::Keyword,
            ),
            ("A.kt", "fun main() { val n = 1 }", "fun", Kind::Keyword),
            (
                "A.swift",
                "func main() { let n = 1 }",
                "func",
                Kind::Keyword,
            ),
            (
                "a.zig",
                "const std = @import(\"std\");",
                "\"std\"",
                Kind::String,
            ),
            (
                "a.dart",
                "void main() { var n = 1; }",
                "main",
                Kind::Function,
            ),
            ("config.toml", "theme = \"dark\"", "theme", Kind::Markup),
            (
                "main.tf",
                "resource \"aws_s3_bucket\" \"b\" {}",
                "resource",
                Kind::Keyword,
            ),
            (
                "default.nix",
                "{ pkgs }: pkgs.hello",
                "pkgs",
                Kind::Property,
            ),
            ("a.ex", "defmodule A do end", "defmodule", Kind::Keyword),
            (
                "a.proto",
                "message A { string name = 1; }",
                "message",
                Kind::Keyword,
            ),
            ("Dockerfile", "FROM alpine:3.19", "FROM", Kind::Keyword),
            (
                "a.graphql",
                "type Query { name: String }",
                "Query",
                Kind::Type,
            ),
            ("a.scss", "$c: red; a { color: $c; }", "red", Kind::Constant),
            (
                "typescript",
                "const total: number = f();",
                "number",
                Kind::Type,
            ),
            ("toml", "theme = \"dark\"", "\"dark\"", Kind::String),
        ];
        for (name, source, anchor, kind) in corpus {
            assert_eq!(kind_of(name, source, anchor), kind, "{name}");
        }
    }
}
