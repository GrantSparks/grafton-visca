//! Source checks for the published unit-test boundary.

use std::path::{Component, Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::LitStr;

/// Checks a source file for dependencies on repository-only code and data.
///
/// The token walk includes macro bodies, where a Rust AST would see tokens
/// rather than the `include_str!` and `include_bytes!` calls they contain.
pub fn violations(path: &Path, source: &str) -> Vec<String> {
    if ![
        "grafton_visca_test_support",
        "include_str",
        "include_bytes",
        "#[path",
        "CARGO_MANIFEST_DIR",
    ]
    .iter()
    .any(|marker| source.contains(marker))
    {
        return Vec::new();
    }
    let tokens: TokenStream = source.parse().expect("tokenize Rust source");
    let mut errors = Vec::new();
    inspect(path, tokens, false, &mut errors);
    errors
}

fn inspect(path: &Path, stream: TokenStream, missing_is_ok: bool, errors: &mut Vec<String>) {
    let tokens: Vec<_> = stream.into_iter().collect();
    let mut remaining = tokens.as_slice();
    while let Some((token, rest)) = remaining.split_first() {
        if ident(token, "match") {
            if let Some((index, arms)) =
                rest.iter()
                    .enumerate()
                    .find_map(|(index, token)| match token {
                        TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                            Some((index, group.stream()))
                        }
                        _ => None,
                    })
            {
                if handles_not_found(arms.clone()) {
                    inspect(path, rest[..index].iter().cloned().collect(), true, errors);
                    inspect(path, arms, missing_is_ok, errors);
                    remaining = &rest[index + 1..];
                    continue;
                }
            }
        }

        if ident(token, "grafton_visca_test_support") {
            errors.push("unpublished integration helper".into());
        }

        if let (TokenTree::Ident(name), [bang, TokenTree::Group(arguments), rest @ ..]) =
            (token, rest)
        {
            if punctuation(bang, '!') {
                let arguments = arguments.stream();
                match name.to_string().as_str() {
                    "include_str" | "include_bytes" => {
                        let literal = syn::parse2::<LitStr>(arguments);
                        let include = literal.as_ref().map(LitStr::value);
                        if !include.is_ok_and(|include| {
                            path.parent()
                                .is_some_and(|parent| shipped(&parent.join(include)))
                        }) {
                            errors.push(format!(
                                "{name}! path must resolve to a shipped file: {}",
                                literal
                                    .map_or_else(|error| error.to_string(), |value| value.value())
                            ));
                        }
                    }
                    "env" | "option_env" if is_manifest_dir(arguments.clone()) => {
                        if !missing_is_ok {
                            errors.push("CARGO_MANIFEST_DIR read needs a NotFound fallback or a shipped path".into());
                        }
                    }
                    "concat" if shipped_manifest_path(arguments.clone()) => {}
                    _ => inspect(path, arguments, missing_is_ok, errors),
                }
                remaining = rest;
                continue;
            }
        }

        if let TokenTree::Group(group) = token {
            inspect(path, group.stream(), missing_is_ok, errors);
        }
        if let [TokenTree::Group(attribute), ..] = rest {
            if punctuation(token, '#') && attribute.delimiter() == Delimiter::Bracket {
                inspect_attribute(path, attribute.stream(), errors);
            }
        }
        remaining = rest;
    }
}

fn inspect_attribute(path: &Path, attribute: TokenStream, errors: &mut Vec<String>) {
    let tokens: Vec<_> = attribute.into_iter().collect();
    if let [name, equals, TokenTree::Literal(value)] = tokens.as_slice() {
        if ident(name, "path") && punctuation(equals, '=') {
            let source = syn::parse2::<LitStr>(TokenTree::Literal(value.clone()).into())
                .expect("literal module path")
                .value();
            if !path
                .parent()
                .is_some_and(|parent| shipped(&parent.join(&source)))
            {
                errors.push(format!("#[path] must resolve to a shipped file: {source}"));
            }
        }
    }
}

fn ident(token: &TokenTree, expected: &str) -> bool {
    matches!(token, TokenTree::Ident(name) if name == expected)
}

fn punctuation(token: &TokenTree, expected: char) -> bool {
    matches!(token, TokenTree::Punct(punctuation) if punctuation.as_char() == expected)
}

fn is_manifest_dir(arguments: TokenStream) -> bool {
    syn::parse2::<LitStr>(arguments).is_ok_and(|value| value.value() == "CARGO_MANIFEST_DIR")
}

fn shipped_manifest_path(arguments: TokenStream) -> bool {
    let tokens: Vec<_> = arguments.into_iter().collect();
    let [name, bang, TokenTree::Group(env), comma, TokenTree::Literal(path)] = tokens.as_slice()
    else {
        return false;
    };
    ident(name, "env")
        && punctuation(bang, '!')
        && is_manifest_dir(env.stream())
        && punctuation(comma, ',')
        && syn::parse2::<LitStr>(TokenTree::Literal(path.clone()).into())
            .ok()
            .and_then(|value| value.value().strip_prefix('/').map(shipped_str))
            == Some(true)
}

fn shipped_str(value: &str) -> bool {
    shipped(Path::new(value))
}

fn shipped(path: &Path) -> bool {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return false,
        }
    }

    let first = normalized.components().next();
    let Some(Component::Normal(first)) = first else {
        return false;
    };
    let first = first.to_str().unwrap_or_default();
    match first {
        "src" | "testdata" | "docs" | "examples" => normalized.components().count() > 1,
        "README.md" | "CONTRIBUTING.md" | "CHANGELOG.md" | "Cargo.toml" | "LICENSE-MIT"
        | "LICENSE-APACHE" => normalized == Path::new(first),
        _ => false,
    }
}

fn handles_not_found(arms: TokenStream) -> bool {
    let arms: Vec<_> = arms.into_iter().collect();
    arms.windows(6).any(|tokens| {
        ident(&tokens[0], "ErrorKind")
            && punctuation(&tokens[1], ':')
            && punctuation(&tokens[2], ':')
            && ident(&tokens[3], "NotFound")
            && punctuation(&tokens[4], '=')
            && punctuation(&tokens[5], '>')
    })
}
