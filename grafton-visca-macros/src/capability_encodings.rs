//! Generates multi-encoding profile data from the typed-support registry.

use std::collections::BTreeSet;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    braced, bracketed,
    parse::{Parse, ParseStream},
    Error, LitStr, Result, Token,
};

struct Variant {
    name: Ident,
    tag: LitStr,
}

struct Family {
    surface: Ident,
    variants: Vec<Variant>,
}

struct Registry {
    families: Vec<Family>,
}

impl Parse for Registry {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let entries;
        bracketed!(entries in input);
        let mut families = Vec::new();
        let mut surfaces = BTreeSet::new();
        while !entries.is_empty() {
            let entry;
            braced!(entry in entries);
            let mut surface = None;
            let mut variants = None;
            let mut fields = BTreeSet::new();
            while !entry.is_empty() {
                let key: Ident = entry.parse()?;
                if !fields.insert(key.to_string()) {
                    return Err(Error::new_spanned(
                        key,
                        "duplicate typed-support registry field",
                    ));
                }
                entry.parse::<Token![:]>()?;
                match key.to_string().as_str() {
                    "surface" => surface = Some(entry.parse::<Ident>()?),
                    "marker" => {
                        entry.parse::<Ident>()?;
                    }
                    "encodings" => {
                        let choices;
                        bracketed!(choices in entry);
                        let mut names = BTreeSet::new();
                        let mut tags = BTreeSet::new();
                        let mut values = Vec::new();
                        while !choices.is_empty() {
                            let name: Ident = choices.parse()?;
                            choices.parse::<Token![=>]>()?;
                            let tag: LitStr = choices.parse()?;
                            if !names.insert(name.to_string()) {
                                return Err(Error::new_spanned(name, "duplicate encoding variant"));
                            }
                            let value = tag.value();
                            if value.is_empty()
                                || !value.chars().all(|ch| {
                                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'
                                })
                            {
                                return Err(Error::new_spanned(
                                    tag,
                                    "encoding tags must be nonempty snake_case identifiers",
                                ));
                            }
                            if !tags.insert(value) {
                                return Err(Error::new_spanned(tag, "duplicate encoding tag"));
                            }
                            values.push(Variant { name, tag });
                            if !choices.is_empty() {
                                choices.parse::<Token![,]>()?;
                            }
                        }
                        if values.len() < 2 {
                            return Err(Error::new_spanned(key, "a single encoding declares no encoding family; multi-encoding surfaces require at least two variants"));
                        }
                        variants = Some(values);
                    }
                    "area" | "surface_doc" | "marker_doc" | "diagnostic" => {
                        entry.parse::<LitStr>()?;
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            key,
                            "unknown typed-support registry field",
                        ))
                    }
                }
                if !entry.is_empty() {
                    entry.parse::<Token![,]>()?;
                }
            }
            let surface = surface.ok_or_else(|| {
                Error::new(entry.span(), "typed-support entry requires a surface")
            })?;
            if !surfaces.insert(surface.to_string()) {
                return Err(Error::new_spanned(
                    surface,
                    "duplicate typed-support surface",
                ));
            }
            if let Some(variants) = variants {
                families.push(Family { surface, variants });
            }
            if !entries.is_empty() {
                entries.parse::<Token![,]>()?;
            }
        }
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after typed-support registry"));
        }
        Ok(Self { families })
    }
}

fn snake_case(ident: &Ident) -> Ident {
    let name = ident.to_string();
    let mut result = String::new();
    for (index, letter) in name.chars().enumerate() {
        if index != 0 && letter.is_ascii_uppercase() {
            result.push('_');
        }
        result.push(letter.to_ascii_lowercase());
    }
    Ident::new(&result, ident.span())
}

pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let Registry { families } = syn::parse2(input)?;
    let path = crate::crate_path::grafton_visca();
    let serde_path = LitStr::new(
        &format!("{path}::__macro_support::serde").replace(' ', ""),
        Span::call_site(),
    );
    let schema_path = LitStr::new(
        &format!("{path}::__macro_support::schemars").replace(' ', ""),
        Span::call_site(),
    );
    let ts_path = LitStr::new(
        &format!("{path}::__macro_support::ts_rs").replace(' ', ""),
        Span::call_site(),
    );
    let derives = quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(#path::__macro_support::serde::Serialize, #path::__macro_support::serde::Deserialize), serde(crate = #serde_path))]
        #[cfg_attr(feature = "schemars", derive(#path::__macro_support::schemars::JsonSchema), schemars(crate = #schema_path))]
        #[cfg_attr(feature = "ts-rs", derive(#path::__macro_support::ts_rs::TS), ts(crate = #ts_path, export))]
    };
    let mut enums = Vec::new();
    let mut selection_variants = Vec::new();
    let mut fields = Vec::new();
    let mut empty_fields = Vec::new();
    let mut getters = Vec::new();
    let mut conversions = Vec::new();
    let mut assignments = Vec::new();
    let mut surfaces = Vec::new();
    let mut iter = Vec::new();
    let mut validators = Vec::new();
    let mut lookups = Vec::new();
    for Family { surface, variants } in &families {
        let encoding = format_ident!("{surface}Encoding", span = surface.span());
        let field = snake_case(surface);
        let field_label = LitStr::new(&format!("encodings.{field}"), field.span());
        let description = format!("The documented encodings of the {surface} surface.");
        let variant_definitions = variants.iter().map(|Variant { name, tag }| {
            let doc = format!("The source-backed `{tag}` encoding.", tag = tag.value());
            quote! { #[doc = #doc] #[cfg_attr(feature = "serde", serde(rename = #tag))] #name, }
        });
        enums.push(quote! {
            #[doc = #description]
            #derives
            #[non_exhaustive]
            pub enum #encoding { #(#variant_definitions)* }
        });
        let doc = format!("Selects an encoding for {surface}.");
        selection_variants.push(quote! { #[doc = #doc] #surface(#encoding), });
        fields.push(quote! {
            #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "::core::option::Option::is_none"))]
            #field: ::core::option::Option<#encoding>,
        });
        empty_fields.push(quote!(#field: ::core::option::Option::None,));
        let doc = format!("Returns the selected encoding for {surface}, if granted.");
        getters.push(quote! {
            #[doc = #doc]
            #[must_use]
            pub const fn #field(&self) -> ::core::option::Option<#encoding> { self.#field }
        });
        conversions.push(quote! {
            impl ::core::convert::From<#encoding> for EncodingSelection {
                fn from(value: #encoding) -> Self { Self::#surface(value) }
            }
        });
        assignments.push(quote!(EncodingSelection::#surface(value) => self.#field = ::core::option::Option::Some(value),));
        surfaces
            .push(quote!(Self::#surface(_) => #path::capabilities::TypedSupportSurface::#surface,));
        iter.push(quote!(self.#field.map(EncodingSelection::#surface),));
        validators.push(quote! {
            let granted = grants.contains(#path::capabilities::TypedSupportSurface::#surface);
            match (granted, self.#field.is_some()) {
                (true, false) => return ::core::result::Result::Err(#path::Error::invalid_parameter(#field_label, "absent", "a granted multi-encoding surface requires an encoding selection")),
                (false, true) => return ::core::result::Result::Err(#path::Error::invalid_parameter(#field_label, "selected", "an encoding selection requires its typed-support grant")),
                _ => {}
            }
        });
        lookups.push(quote! {
            (#surface($variant:ident)) => { #path::capabilities::EncodingSelection::#surface(#path::capabilities::#encoding::$variant) };
        });
    }
    let count = families.len();
    Ok(quote! {
        #(#enums)*

        /// One strongly typed selection for a multi-encoding capability.
        #derives
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
        #[non_exhaustive]
        pub enum EncodingSelection { #(#selection_variants)* }

        impl EncodingSelection {
            /// Returns the capability surface this selection belongs to.
            #[must_use]
            pub const fn surface(self) -> #path::capabilities::TypedSupportSurface {
                match self { #(#surfaces)* }
            }
        }

        #(#conversions)*

        /// Inline encoding selections keyed by their typed capability surface.
        ///
        /// Only multi-encoding surfaces have entries. A profile validates each
        /// entry together with the corresponding typed-support grant.
        #derives
        #[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
        #[non_exhaustive]
        pub struct ProfileEncodings { #(#fields)* }

        impl ProfileEncodings {
            /// No multi-encoding selections.
            pub const EMPTY: Self = Self { #(#empty_fields)* };

            #(#getters)*

            /// Returns this value with one selected capability encoding.
            #[must_use]
            pub const fn with_encoding(mut self, selection: EncodingSelection) -> Self {
                match selection { #(#assignments)* }
                self
            }

            /// Iterates over selected encodings without allocating.
            pub fn iter(&self) -> impl ::core::iter::Iterator<Item = EncodingSelection> {
                let selections: [::core::option::Option<EncodingSelection>; #count] = [#(#iter)*];
                selections.into_iter().flatten()
            }

            /// Requires an encoding exactly when its capability is granted.
            pub(crate) fn validate(&self, grants: #path::capabilities::TypedSupportSet) -> #path::Result<()> {
                #(#validators)*
                ::core::result::Result::Ok(())
            }
        }

        impl ::core::default::Default for ProfileEncodings {
            fn default() -> Self { Self::EMPTY }
        }

        macro_rules! encoding_selection { #(#lookups)* }
        pub(crate) use encoding_selection;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_registry_surface_generates_its_enum_field_and_lookup() {
        let output = expand(quote!([
            {surface: PresetRecallSpeed, marker: HasPresetRecallSpeed,
             encodings: [PtzOptics => "ptzoptics", SonyFr7 => "sony_fr7"],
             area: "Presets", surface_doc: "Speed", marker_doc: "Evidence", diagnostic: "Unsupported"},
            {surface: DirectZoom, marker: HasDirectZoom,
             area: "Zoom", surface_doc: "Zoom", marker_doc: "Evidence", diagnostic: "Unsupported"},
        ])).unwrap().to_string();
        assert!(output.contains("pub enum PresetRecallSpeedEncoding"));
        assert!(output.contains("pub const fn preset_recall_speed"));
        assert!(output.contains("ptzoptics"));
        assert!(output.contains("encoding_selection"));
        assert!(!output.contains("DirectZoomEncoding"));
    }

    #[test]
    fn malformed_encoding_families_are_rejected() {
        for choices in [
            quote!(Only => "only"),
            quote!(A => "same", B => "same"),
            quote!(A => "a", A => "other"),
            quote!(A => "UpperCase", B => "b"),
        ] {
            assert!(expand(
                quote!([{surface: Surface, marker: HasSurface, encodings: [#choices]}])
            )
            .is_err());
        }
    }
}
