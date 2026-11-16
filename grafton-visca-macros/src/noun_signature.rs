//! Derives prelude value imports from the noun and motion method signatures.

use std::collections::BTreeMap;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{
    braced, bracketed,
    fold::{self, Fold},
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Attribute, Error, Expr, FnArg, Path, Result, ReturnType, Signature, Token, Type,
};

struct Imports {
    crate_path: Path,
    types: BTreeMap<String, Ident>,
    operation: BTreeMap<String, Ident>,
}

impl Imports {
    fn signature_type(&mut self, ty: Type) {
        // Folding only the parsed type visits nested generics, associated
        // types, function types and const expressions, without reaching a
        // constructor or any header metadata.
        self.fold_type(ty);
    }

    fn arguments(&mut self, arguments: impl IntoIterator<Item = FnArg>) {
        for argument in arguments {
            if let FnArg::Typed(argument) = argument {
                self.signature_type(*argument.ty);
            }
        }
    }

    fn table(&mut self, input: ParseStream<'_>) -> Result<()> {
        while !input.is_empty() {
            let _attributes = input.call(Attribute::parse_outer)?;
            if input.peek(Token![@]) {
                input.parse::<Token![@]>()?;
                let header: Ident = input.parse()?;
                if header == "exceptions" {
                    input.parse::<Token![;]>()?;
                    // Protocol exceptions have no callable noun signature.
                    input.parse::<TokenStream>()?;
                    break;
                }
                if header == "noun" {
                    input.parse::<Ident>()?;
                } else if header != "motion" {
                    return Err(Error::new_spanned(
                        header,
                        "expected noun or motion table header",
                    ));
                }
                let fields;
                braced!(fields in input);
                fields.parse::<TokenStream>()?;
                input.parse::<Token![;]>()?;
            } else if input.peek(Token![fn]) {
                let signature: Signature = input.parse()?;
                self.arguments(signature.inputs);
                if let ReturnType::Type(_, ty) = signature.output {
                    self.signature_type(*ty);
                }
                input.parse::<Token![=>]>()?;
                input.parse::<Expr>()?;
                input.parse::<Token![;]>()?;
            } else {
                let kind: Ident = input.parse()?;
                if !matches!(
                    kind.to_string().as_str(),
                    "inquiry" | "plain" | "applied" | "targeted"
                ) {
                    return Err(Error::new_spanned(kind, "expected a noun method row"));
                }
                // Command rows spell the underlying request as their return
                // type; the facade projects it to an operation handle whose
                // completion kind is fixed by this row classification.
                let completion = if kind == "applied" {
                    Some("AppliedOnly")
                } else if kind == "targeted" {
                    Some("Targeted")
                } else {
                    None
                };
                if let Some(completion) = completion {
                    self.operation
                        .entry(completion.to_owned())
                        .or_insert_with(|| Ident::new(completion, kind.span()));
                }
                let key;
                bracketed!(key in input);
                key.parse::<TokenStream>()?;
                input.parse::<Ident>()?;
                let arguments;
                parenthesized!(arguments in input);
                self.arguments(arguments.parse_terminated(FnArg::parse, Token![,])?);
                input.parse::<Token![->]>()?;
                self.signature_type(input.parse()?);
                if input.peek(Token![where]) {
                    input.parse::<Token![where]>()?;
                    Punctuated::<Ident, Token![+]>::parse_separated_nonempty(input)?;
                }
                input.parse::<Token![=]>()?;
                let constructor;
                bracketed!(constructor in input);
                constructor.parse::<TokenStream>()?;
                input.parse::<Token![;]>()?;
            }
        }
        Ok(())
    }
}

impl Imports {
    fn canonical_type(&self, path: &Path, home: &str) -> Option<Ident> {
        let segments: Vec<_> = path.segments.iter().collect();
        let base: Vec<_> = self.crate_path.segments.iter().collect();
        let offset = if segments
            .first()
            .is_some_and(|segment| segment.ident == home)
        {
            Some(0)
        } else if segments
            .first()
            .is_some_and(|segment| segment.ident == "crate")
            && segments.get(1).is_some_and(|segment| segment.ident == home)
        {
            Some(1)
        } else if segments.len() > base.len()
            && segments
                .iter()
                .zip(&base)
                .all(|(actual, expected)| actual.ident == expected.ident)
            && segments[base.len()].ident == home
        {
            Some(base.len())
        } else {
            None
        };
        offset
            .and_then(|offset| segments.get(offset + 1))
            .map(|segment| segment.ident.clone())
    }
}

impl Fold for Imports {
    fn fold_path(&mut self, path: Path) -> Path {
        if let Some(ident) = self.canonical_type(&path, "types") {
            self.types.entry(ident.to_string()).or_insert(ident);
        }
        if let Some(ident) = self.canonical_type(&path, "operation") {
            self.operation.entry(ident.to_string()).or_insert(ident);
        }
        fold::fold_path(self, path)
    }
}

impl Parse for Imports {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![crate]>()?;
        input.parse::<Token![=]>()?;
        let mut imports = Self {
            crate_path: input.parse()?,
            types: BTreeMap::new(),
            operation: BTreeMap::new(),
        };
        input.parse::<Token![;]>()?;
        if input.is_empty() {
            return Err(input.error("expected a bracketed noun or motion table"));
        }
        while !input.is_empty() {
            let table;
            bracketed!(table in input);
            imports.table(&table)?;
        }
        Ok(imports)
    }
}

pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let Imports {
        crate_path,
        types,
        operation,
    } = syn::parse2(input)?;
    let values = (!types.is_empty()).then(|| {
        let types = types.values();
        quote!(pub use #crate_path::types::{#(#types),*};)
    });
    let operation = (!operation.is_empty()).then(|| {
        let operation = operation.values();
        quote!(pub use #crate_path::operation::{#(#operation),*};)
    });
    Ok(quote! {
        #values
        #operation
        pub use ::std::time::Duration;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Delimiter, TokenTree};

    #[test]
    fn only_method_signatures_contribute_deduplicated_value_imports() {
        let output = expand(quote! {
            crate = ::renamed;
            [
                @noun Zoom { doc: "types::HeaderGhost", value: types::HeaderGhost };
                /// types::DocGhost
                targeted [SetPosition] set_position(
                    speed: types::Speed<types::PanTiltSpeed>,
                    position: ::renamed::types::ZoomPosition,
                    callback: fn(crate::types::FocusPosition) -> types::ZoomPosition,
                ) -> Result<Option<types::UnitInterval>, OtherError> where HasZoom + HasDirectZoom
                    = [types::ConstructorGhost::new(types::OnlyInConstructor)];
                inquiry [PositionInquiry] position() -> types::ZoomPosition
                    = [command::zoom::PositionInquiry];
                @exceptions;
                internal [Hidden] hidden -> types::ProtocolGhost;
            ]
            [
                @motion { doc: "types::MotionHeaderGhost" };
                fn wait(&self, wait: types::IdleWait) -> Result<types::UnitInterval, Error>
                    => wait(types::MotionConstructorGhost);
            ]
        })
        .unwrap()
        .to_string();
        assert_eq!(output, quote! {
            pub use ::renamed::types::{FocusPosition, IdleWait, PanTiltSpeed, Speed, UnitInterval, ZoomPosition};
            pub use ::renamed::operation::{Targeted};
            pub use ::std::time::Duration;
        }.to_string());
    }

    #[test]
    fn nested_signature_values_are_not_lost_or_taken_from_foreign_modules() {
        let output = expand(quote! {
            crate = crate;
            [inquiry [Query] query(
                values: [types::GainLevel; types::Count::VALUE],
                nested: <types::Level as Trait<types::Other>>::Output,
                foreign: vendor::types::Foreign,
                outcome: operation::Settlement,
                foreign_outcome: vendor::operation::ForeignOutcome,
            ) -> (types::GainLevel, &'static types::Borrowed) = [command::Query];]
        })
        .unwrap()
        .to_string();
        assert_eq!(
            output,
            quote! {
                pub use crate::types::{Borrowed, Count, GainLevel, Level, Other};
                pub use crate::operation::{Settlement};
                pub use ::std::time::Duration;
            }
            .to_string()
        );
    }

    #[test]
    fn facade_operation_kinds_and_explicit_operation_signatures_are_closed() {
        let output = expand(quote! {
            crate = ::renamed;
            [
                @noun Zoom { hidden: operation::HeaderGhost };
                applied [Tele] tele() -> command::zoom::Tele
                    = [operation::ConstructorGhost::new()];
                applied [Wide] wide() -> command::zoom::Wide = [command::zoom::Wide];
                targeted [Set] set() -> command::zoom::Set = [command::zoom::Set];
                @exceptions;
                internal [Hidden] hidden -> operation::ProtocolGhost;
            ]
            [
                @motion { hidden: operation::MotionHeaderGhost };
                fn stop_all(&self) -> Result<crate::operation::HaltReport, Error>
                    => inspect(operation::MotionConstructorGhost);
            ]
        })
        .unwrap()
        .to_string();
        assert_eq!(
            output,
            quote! {
                pub use ::renamed::operation::{AppliedOnly, HaltReport, Targeted};
                pub use ::std::time::Duration;
            }
            .to_string()
        );
        let plain = expand(quote! {
            crate = crate;
            [plain [Set] set() -> command::Set = [operation::ConstructorGhost];]
        })
        .unwrap()
        .to_string();
        assert_eq!(
            plain,
            quote!(
                pub use ::std::time::Duration;
            )
            .to_string()
        );
    }

    fn actual_table(name: &str) -> Option<TokenStream> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/noun_table.rs");
        // Published macro packages have no sibling main library. The literal
        // grammar fixtures above remain available in that standalone package.
        if !path.exists() {
            return None;
        }
        let source = syn::parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();
        let definition = source
            .items
            .into_iter()
            .find_map(|item| match item {
                syn::Item::Macro(item)
                    if item.ident.as_ref().is_some_and(|ident| ident == name) =>
                {
                    Some(item.mac.tokens)
                }
                _ => None,
            })
            .unwrap();
        let body = definition
            .into_iter()
            .find_map(|token| match token {
                TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                    Some(group.stream())
                }
                _ => None,
            })
            .unwrap();
        let callback = body
            .into_iter()
            .find_map(|token| match token {
                TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                    Some(group.stream())
                }
                _ => None,
            })
            .unwrap();
        Some(
            callback
                .into_iter()
                .skip_while(
                    |token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == '@'),
                )
                .collect(),
        )
    }

    #[test]
    fn the_actual_noun_and_motion_tables_share_one_closed_import_set() {
        let (Some(nouns), Some(motion)) =
            (actual_table("noun_table"), actual_table("motion_table"))
        else {
            return;
        };
        let imports: Imports = syn::parse2(quote!(crate = ::renamed; [#nouns] [#motion])).unwrap();
        assert!(imports.types.len() > 50);
        for value in [
            "BlackLevel",
            "FocusPosition",
            "IdleWait",
            "MotionQuery",
            "Speed",
            "PanTiltSpeed",
            "UnitInterval",
        ] {
            assert!(imports.types.contains_key(value), "missing {value}");
        }
        assert!(!imports.types.contains_key("PositionInquiry"));
        assert!(!imports.types.contains_key("SetPosition"));
        assert_eq!(
            imports
                .operation
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["AppliedOnly", "HaltReport", "Targeted"]
        );
    }

    #[test]
    fn malformed_tables_fail_instead_of_silently_omitting_signature_types() {
        assert!(expand(quote!(crate = crate;)).is_err());
        assert!(expand(quote!(crate = crate; [@unknown {};])).is_err());
        assert!(expand(
            quote!(crate = crate; [plain [Set] set(value: types::Level) -> () = [request]])
        )
        .is_err());
    }
}
