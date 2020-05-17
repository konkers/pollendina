#![allow(dead_code)]
#![allow(unused_variables)]
extern crate proc_macro;

use proc_macro_hack::proc_macro_hack;
use quote::{format_ident, quote};
use syn::parse_macro_input;

use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Path, Result, Token, Type};

enum PathOrWildcard {
    Path(Path),
    Wildcard,
}

struct WidgetMatch {
    subject: Path,
    branches: Vec<MatchBranch>,
}

struct MatchBranch {
    variant: PathOrWildcard,
    params: Vec<Type>,
    expr: Expr,
}

impl Parse for PathOrWildcard {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![_]) {
            input.parse::<Token![_]>()?;
            Ok(PathOrWildcard::Wildcard)
        } else {
            let path: Path = input.parse()?;
            Ok(PathOrWildcard::Path(path))
        }
    }
}

impl Parse for WidgetMatch {
    fn parse(input: ParseStream) -> Result<Self> {
        let subject = input.parse()?;
        input.parse::<Token![,]>()?;

        let branches = input
            .parse_terminated::<MatchBranch, Token![,]>(MatchBranch::parse)?
            .into_iter()
            .collect();

        Ok(WidgetMatch { subject, branches })
    }
}

impl Parse for MatchBranch {
    fn parse(input: ParseStream) -> Result<Self> {
        let variant = PathOrWildcard::parse(input)?;

        let params = if input.peek(syn::token::Paren) {
            let types;
            syn::parenthesized!(types in input);
            Punctuated::<Type, Token![,]>::parse_separated_nonempty(&types)?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        input.parse::<Token![=>]>()?;

        let expr = input.parse()?;

        Ok(MatchBranch {
            variant,
            params,
            expr,
        })
    }
}

#[proc_macro_hack]
pub fn match_widget(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let wm: WidgetMatch = parse_macro_input!(input as WidgetMatch);

    let target = wm.subject;

    let branches = wm.branches.into_iter().map(|branch: MatchBranch| {
        let variant = match branch.variant {
            PathOrWildcard::Path(path) => quote!(#path),
            PathOrWildcard::Wildcard => quote!(_),
        };
        let expr = branch.expr;

        let wtype = if branch.params.is_empty() {
            quote! { () }
        } else {
            let params = branch.params.iter().cloned();
            quote! {
                (#(#params),*)
            }
        };

        let param_count = branch.params.len();
        let param_names: Vec<syn::Ident> = branch
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| format_ident!("a{}", i))
            .collect();

        let assignments = param_names
            .iter()
            .enumerate()
            .map(|(i, name)| (syn::Index::from(i), name))
            .map(|(i, name)| {
                if param_count == 1 {
                    quote! { *#name = new }
                } else {
                    quote! { *#name = new.#i }
                }
            });

        let lens = if branch.params.is_empty() {
            quote! {
                druid::lens::Map::new(
                    |data: &#target| match data {
                        #variant => (),
                        _ => unreachable!(),
                    },
                    |data: &mut #target, new: #wtype| match data {
                        #variant => (),
                        _ => unreachable!(),
                    }
                )
            }
        } else {
            let pn1 = param_names.iter();
            let pn2 = param_names.iter();
            let pn3 = param_names.iter();
            quote! {
                druid::lens::Map::new(
                    |data: &#target| match data {
                        #variant(#(#pn1),*) => (#(#pn2.clone()),*),
                        _ => unreachable!(),
                    },
                    |data: &mut #target, new: #wtype| match data {
                        #variant(#(#pn3),*) => {
                            #(#assignments);*
                        },
                        _ => unreachable!(),
                    }
                )
            }
        };

        let result = quote! {
            {
                let widget = #expr;
                let lensed = widget.lens(#lens);
                let boxed: Box<dyn druid::Widget<#target>> = Box::new(lensed);
                boxed
            }
        };

        if branch.params.is_empty() {
            quote! {
                #variant => #result
            }
        } else {
            let pattern = branch.params.iter().map(|_| quote!(_));
            quote! {
                #variant(#(#pattern),*) => #result
            }
        }
    });

    let output = quote! {
        match_macro::WidgetMatcher::new(|target: &#target| match target {
            #(#branches,)*
        })
    };

    proc_macro::TokenStream::from(output)
}
