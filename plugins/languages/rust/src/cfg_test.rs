use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, File, Item, Meta};

pub(crate) fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_cfg_test_attr)
}

pub(crate) fn is_cfg_attr_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg_attr") {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    let condition: Vec<TokenTree> = list
        .tokens
        .clone()
        .into_iter()
        .take_while(|token| !is_comma(token))
        .collect();
    is_cfg_test_predicate(&condition)
}

fn is_cfg_test_attr(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    let tokens: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
    is_cfg_test_predicate(&tokens)
}

fn is_cfg_test_predicate(tokens: &[TokenTree]) -> bool {
    match tokens {
        [TokenTree::Ident(ident)] => ident == "test",
        [TokenTree::Ident(ident), TokenTree::Group(group)]
            if ident == "all" && group.delimiter() == Delimiter::Parenthesis =>
        {
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            inner.split(is_comma).any(is_cfg_test_predicate)
        }
        _ => false,
    }
}

fn is_comma(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == ',')
}

pub(crate) fn excluded_cfg_test_lines(file: &File) -> BTreeSet<u32> {
    let mut collector = CfgTestLineCollector {
        lines: BTreeSet::new(),
    };
    collector.visit_file(file);
    collector.lines
}

struct CfgTestLineCollector {
    lines: BTreeSet<u32>,
}

impl CfgTestLineCollector {
    fn record_span(&mut self, span: proc_macro2::Span) {
        let start = span.start().line as u32;
        let end = span.end().line as u32;
        for line in start..=end {
            self.lines.insert(line);
        }
    }

    fn record_attrs_and_item(&mut self, attrs: &[Attribute], item: &Item) {
        for attr in attrs {
            self.record_span(attr.span());
        }
        self.record_span(item.span());
    }
}

impl<'ast> Visit<'ast> for CfgTestLineCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        let attrs = item_attrs(item);
        if has_cfg_test(attrs) {
            self.record_attrs_and_item(attrs, item);
            return;
        }
        visit::visit_item(self, item);
    }
}

pub(crate) fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}
