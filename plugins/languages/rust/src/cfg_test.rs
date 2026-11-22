use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, File, Item, Meta};

pub(crate) fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_cfg_test_attr)
}

fn is_cfg_test_attr(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    let mut found_test = false;
    let _ = list.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            found_test = true;
        }
        Ok(())
    });
    found_test
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
