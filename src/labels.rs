//! Colored labels for RobotCode's completions and symbols.
//!
//! Zed highlights a label by parsing `code` with the Robot Framework grammar
//! and showing the `code_range` spans, so each label is placed in a small
//! piece of Robot code where the grammar gives it the right color.

use zed_extension_api::{
    lsp::{Completion, CompletionKind, Symbol, SymbolKind},
    CodeLabel, CodeLabelSpan,
};

const IN_TEST: &str = "*** Test Cases ***\nT\n    ";

pub fn completion(completion: &Completion) -> Option<CodeLabel> {
    let label = completion.label.as_str();
    match completion.kind.as_ref()? {
        // Keywords: "Keyword (BuiltIn)" in `detail` names the library.
        CompletionKind::Function => {
            let mut code_label = in_code(IN_TEST, label, "\n");
            if let Some(library) = completion
                .detail
                .as_deref()
                .and_then(|d| d.split_once('('))
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .filter(|lib| !lib.is_empty())
            {
                code_label.spans.push(CodeLabelSpan::literal(
                    format!("  {library}"),
                    Some("comment".into()),
                ));
            }
            Some(code_label)
        }
        // `${var}`, `@{list}`, `&{dict}`, `%{ENV}`; named arguments (`timeout=`)
        // and environment variable names come without braces.
        CompletionKind::Variable if is_variable(label) => {
            Some(in_code(&format!("{IN_TEST}Log    "), label, "\n"))
        }
        CompletionKind::Variable => Some(literal(label, "variable")),
        // Section headers.
        CompletionKind::Class if label.starts_with('*') => Some(in_code("", label, "\n")),
        // Settings (`Library`, `[Tags]`) and markers like `AS` or `NONE`.
        CompletionKind::Keyword if label.starts_with('[') => Some(literal(label, "attribute")),
        CompletionKind::Keyword => Some(literal(label, "keyword")),
        // Libraries and resources.
        CompletionKind::Module => Some(literal(label, "type")),
        _ => None,
    }
}

pub fn symbol(symbol: &Symbol) -> Option<CodeLabel> {
    let name = symbol.name.as_str();
    match symbol.kind {
        SymbolKind::Function => Some(in_code("*** Keywords ***\n", name, "\n")),
        // Test cases and tasks.
        SymbolKind::Class => Some(in_code("*** Test Cases ***\n", name, "\n")),
        SymbolKind::Variable if is_variable(name) => {
            Some(in_code("*** Variables ***\n", name, "    x\n"))
        }
        SymbolKind::Variable => Some(literal(name, "variable")),
        SymbolKind::Module => Some(literal(name, "type")),
        _ => None,
    }
}

fn is_variable(text: &str) -> bool {
    ["${", "@{", "&{", "%{"].iter().any(|p| text.starts_with(p)) && text.ends_with('}')
}

/// `label` shown with the colors it gets between `before` and `after`.
fn in_code(before: &str, label: &str, after: &str) -> CodeLabel {
    let start = before.len();
    CodeLabel {
        code: format!("{before}{label}{after}"),
        spans: vec![CodeLabelSpan::code_range(
            start as u32..(start + label.len()) as u32,
        )],
        filter_range: whole(label),
    }
}

fn literal(label: &str, highlight: &str) -> CodeLabel {
    CodeLabel {
        code: String::new(),
        spans: vec![CodeLabelSpan::literal(label, Some(highlight.into()))],
        filter_range: whole(label),
    }
}

fn whole(label: &str) -> zed_extension_api::Range {
    (0..label.len() as u32).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, kind: CompletionKind, detail: Option<&str>) -> Completion {
        Completion {
            label: label.into(),
            label_details: None,
            detail: detail.map(String::from),
            kind: Some(kind),
            insert_text_format: None,
        }
    }

    fn shown(label: &CodeLabel) -> String {
        label
            .spans
            .iter()
            .map(|span| match span {
                CodeLabelSpan::CodeRange(r) => {
                    label.code[r.start as usize..r.end as usize].to_string()
                }
                CodeLabelSpan::Literal(l) => l.text.clone(),
            })
            .collect()
    }

    #[test]
    fn keywords_show_their_library() {
        let label = completion(&item(
            "Should Be Equal",
            CompletionKind::Function,
            Some("Keyword (BuiltIn)"),
        ))
        .unwrap();
        assert_eq!(shown(&label), "Should Be Equal  BuiltIn");
        assert_eq!(label.code, "*** Test Cases ***\nT\n    Should Be Equal\n");
        assert_eq!(label.filter_range.end, "Should Be Equal".len() as u32);

        let label = completion(&item(
            "My Keyword",
            CompletionKind::Function,
            Some("Keyword "),
        ))
        .unwrap();
        assert_eq!(shown(&label), "My Keyword");
    }

    #[test]
    fn variables_are_parsed_as_variables() {
        let label = completion(&item("${user}", CompletionKind::Variable, None)).unwrap();
        assert_eq!(shown(&label), "${user}");
        assert!(label.code.ends_with("Log    ${user}\n"));

        let label = completion(&item("timeout=", CompletionKind::Variable, None)).unwrap();
        assert!(matches!(&label.spans[0], CodeLabelSpan::Literal(l) if l.text == "timeout="));
    }

    #[test]
    fn other_completions() {
        let header = completion(&item("*** Settings ***", CompletionKind::Class, None)).unwrap();
        assert_eq!(header.code, "*** Settings ***\n");
        assert!(completion(&item("[Tags]", CompletionKind::Keyword, None)).is_some());
        assert!(completion(&item("SSHLibrary", CompletionKind::Module, None)).is_some());
        assert!(completion(&item("file.robot", CompletionKind::File, None)).is_none());
        let mut no_kind = item("x", CompletionKind::Text, None);
        no_kind.kind = None;
        assert!(completion(&no_kind).is_none());
    }

    #[test]
    fn symbols() {
        let kw = symbol(&Symbol {
            kind: SymbolKind::Function,
            name: "Log In".into(),
        })
        .unwrap();
        assert_eq!(kw.code, "*** Keywords ***\nLog In\n");
        assert_eq!(shown(&kw), "Log In");
        let var = symbol(&Symbol {
            kind: SymbolKind::Variable,
            name: "${HOST}".into(),
        })
        .unwrap();
        assert_eq!(shown(&var), "${HOST}");
        assert!(symbol(&Symbol {
            kind: SymbolKind::File,
            name: "a".into()
        })
        .is_none());
    }
}
