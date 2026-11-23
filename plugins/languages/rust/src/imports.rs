use std::collections::{BTreeSet, HashSet};

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Arm, Attribute, Expr, ExprLit, Field, FieldValue, File, ImplItem, Item, ItemExternCrate,
    ItemMacro, ItemMod, ItemUse, Lit, Local, Macro, Meta, Path, StmtMacro, TraitItem, UseTree,
    Variant,
};

use crate::cfg_test::{has_cfg_test, is_cfg_attr_test, item_attrs};

pub(crate) const MOD_PREFIX: &str = "mod:";
pub(crate) const MOD_PATH_PREFIX: &str = "mod-path:";
pub(crate) const USE_PREFIX: &str = "use:";
pub(crate) const PATH_PREFIX: &str = "path:";

pub(crate) fn runtime_import_specifiers(file: &File, exclude_cfg_test: bool) -> Vec<String> {
    let mut local_collector = LocalIdentCollector {
        exclude_cfg_test,
        idents: HashSet::new(),
        external_mods: HashSet::new(),
    };
    local_collector.visit_file(file);
    let mut collector = ImportCollector {
        exclude_cfg_test,
        local_idents: local_collector.idents,
        external_mods: local_collector.external_mods,
        inline_mod_stack: Vec::new(),
        specifiers: BTreeSet::new(),
    };
    collector.visit_file(file);
    collector.specifiers.into_iter().collect()
}

struct LocalIdentCollector {
    exclude_cfg_test: bool,
    idents: HashSet<String>,
    external_mods: HashSet<String>,
}

impl<'ast> Visit<'ast> for LocalIdentCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.exclude_cfg_test && has_cfg_test(item_attrs(item)) {
            return;
        }
        if let Some(ident) = declared_item_ident(item) {
            self.idents.insert(ident);
        }
        visit::visit_item(self, item);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if node.content.is_some() {
            self.idents.insert(node.ident.to_string());
            visit::visit_item_mod(self, node);
        } else {
            let ident = node.ident.to_string();
            let stripped = ident.strip_prefix("r#").unwrap_or(&ident);
            self.external_mods.insert(stripped.to_owned());
        }
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        let mut prefix = Vec::new();
        collect_use_introduced_idents(&node.tree, &mut prefix, &mut self.idents);
    }

    fn visit_item_extern_crate(&mut self, node: &'ast ItemExternCrate) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if let Some((_, alias)) = &node.rename {
            let alias_str = alias.to_string();
            if alias_str != "_" && node.ident != alias_str {
                self.idents.insert(alias_str);
            }
        }
    }

    fn visit_item_macro(&mut self, node: &'ast ItemMacro) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if is_bindgen_macro(&node.mac.path) {
            self.idents.insert("exports".to_owned());
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.exclude_cfg_test && has_cfg_test(expr_attrs(expr)) {
            return;
        }
        visit::visit_expr(self, expr);
    }
}

fn declared_item_ident(item: &Item) -> Option<String> {
    let ident = match item {
        Item::Enum(node) => &node.ident,
        Item::Struct(node) => &node.ident,
        Item::Trait(node) => &node.ident,
        Item::TraitAlias(node) => &node.ident,
        Item::Type(node) => &node.ident,
        Item::Union(node) => &node.ident,
        _ => return None,
    };
    Some(ident.to_string())
}

fn is_bindgen_macro(path: &Path) -> bool {
    let Some(last) = path.segments.last() else {
        return false;
    };
    let last_name = last.ident.to_string();
    last_name == "bindgen"
        || (last_name == "generate"
            && path
                .segments
                .iter()
                .any(|segment| segment.ident == "wit_bindgen"))
}

fn collect_use_introduced_idents(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    out: &mut HashSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_introduced_idents(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => record_introduced_name(&name.ident.to_string(), prefix, out),
        UseTree::Rename(rename) => record_introduced_rename(rename, prefix, out),
        UseTree::Glob(_) => {}
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_introduced_idents(item, prefix, out);
            }
        }
    }
}

fn record_introduced_name(ident: &str, prefix: &[String], out: &mut HashSet<String>) {
    let Some(root) = prefix.first() else {
        return;
    };
    if ident == "self" {
        if prefix.len() >= 2
            && let Some(leaf) = prefix.last()
            && leaf != root
        {
            out.insert(leaf.clone());
        }
        return;
    }
    if ident != root {
        out.insert(ident.to_owned());
    }
}

fn record_introduced_rename(rename: &syn::UseRename, prefix: &[String], out: &mut HashSet<String>) {
    let alias = rename.rename.to_string();
    if alias == "_" {
        return;
    }
    let same_as_root = prefix
        .first()
        .map_or_else(|| rename.ident == alias, |root| root == &alias);
    if !same_as_root {
        out.insert(alias);
    }
}

struct ImportCollector {
    exclude_cfg_test: bool,
    local_idents: HashSet<String>,
    external_mods: HashSet<String>,
    inline_mod_stack: Vec<String>,
    specifiers: BTreeSet<String>,
}

macro_rules! visit_unless_cfg_test {
    ($($method:ident($ty:ty, $attrs:ident);)*) => {
        $(
            fn $method(&mut self, node: &'ast $ty) {
                if !self.skip_cfg_test($attrs(node)) {
                    visit::$method(self, node);
                }
            }
        )*
    };
    (field; $($method:ident($ty:ty);)*) => {
        $(
            fn $method(&mut self, node: &'ast $ty) {
                if !self.skip_cfg_test(&node.attrs) {
                    visit::$method(self, node);
                }
            }
        )*
    };
}

macro_rules! match_expr_attrs {
    ($expr:expr, [$($variant:ident),* $(,)?]) => {
        match $expr {
            $(Expr::$variant(node) => &node.attrs,)*
            _ => &[],
        }
    };
}

impl ImportCollector {
    fn skip_cfg_test(&self, attrs: &[Attribute]) -> bool {
        self.exclude_cfg_test && has_cfg_test(attrs)
    }

    fn record_external_mod(&mut self, node: &ItemMod) {
        if let Some(rel_path) = mod_path_attr(&node.attrs) {
            let full = if self.inline_mod_stack.is_empty() || rel_path.starts_with('/') {
                rel_path
            } else {
                format!("{}/{rel_path}", self.inline_mod_stack.join("/"))
            };
            self.specifiers.insert(format!("{MOD_PATH_PREFIX}{full}"));
            return;
        }
        let ident = node.ident.to_string();
        let name = ident.strip_prefix("r#").unwrap_or(&ident);
        if self.inline_mod_stack.is_empty() {
            self.specifiers.insert(format!("{MOD_PREFIX}{name}"));
        } else {
            self.specifiers.insert(format!(
                "{MOD_PREFIX}{}::{name}",
                self.inline_mod_stack.join("::")
            ));
        }
    }

    fn record_qualified_path(&mut self, path: &Path) {
        if path.segments.len() < 2 {
            return;
        }
        let raw_head = path.segments[0].ident.to_string();
        self.record_qualified_head(&raw_head, path.leading_colon.is_some());
    }

    fn record_qualified_head(&mut self, raw_head: &str, leading_colon: bool) {
        let head = raw_head.strip_prefix("r#").unwrap_or(raw_head);
        let Some(first) = head.bytes().next() else {
            return;
        };
        if !(first.is_ascii_lowercase() || first == b'_') || head == "_" {
            return;
        }
        if is_ignored_path_head(head) {
            return;
        }
        if !leading_colon && (self.local_idents.contains(head) || self.external_mods.contains(head))
        {
            return;
        }
        self.specifiers.insert(format!("{PATH_PREFIX}{head}"));
    }

    fn scan_token_stream(&mut self, stream: TokenStream) {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        for (idx, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Group(group) => self.scan_token_stream(group.stream()),
                TokenTree::Ident(ident) => self.inspect_ident_token(&tokens, idx, ident),
                _ => {}
            }
        }
    }

    fn inspect_ident_token(
        &mut self,
        tokens: &[TokenTree],
        idx: usize,
        ident: &proc_macro2::Ident,
    ) {
        if !is_double_colon(tokens, idx + 1)
            || !matches!(tokens.get(idx + 3), Some(TokenTree::Ident(_)))
        {
            return;
        }
        if is_path_continuation(tokens, idx) {
            return;
        }
        let leading_colon = idx >= 2 && is_double_colon(tokens, idx - 2);
        self.record_qualified_head(&ident.to_string(), leading_colon);
    }
}

impl<'ast> Visit<'ast> for ImportCollector {
    visit_unless_cfg_test! {
        visit_item(Item, item_attrs);
        visit_impl_item(ImplItem, impl_item_attrs);
        visit_trait_item(TraitItem, trait_item_attrs);
        visit_expr(Expr, expr_attrs);
    }

    visit_unless_cfg_test! {
        field;
        visit_field(Field);
        visit_field_value(FieldValue);
        visit_variant(Variant);
        visit_local(Local);
        visit_stmt_macro(StmtMacro);
        visit_arm(Arm);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if node.content.is_some() {
            let segment = mod_path_attr(&node.attrs).unwrap_or_else(|| node.ident.to_string());
            self.inline_mod_stack.push(segment);
            visit::visit_item_mod(self, node);
            self.inline_mod_stack.pop();
        } else {
            for attr in &node.attrs {
                self.visit_attribute(attr);
            }
            self.record_external_mod(node);
        }
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        for attr in &node.attrs {
            self.visit_attribute(attr);
        }
        let allow_local_filter = node.leading_colon.is_none();
        let mut prefix = Vec::new();
        collect_use_tree(
            &node.tree,
            &mut prefix,
            allow_local_filter,
            &self.local_idents,
            &mut self.specifiers,
        );
    }

    fn visit_item_extern_crate(&mut self, node: &'ast ItemExternCrate) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        let ident = node.ident.to_string();
        if ident != "self" {
            self.specifiers.insert(format!("{USE_PREFIX}{ident}"));
        }
    }

    fn visit_path(&mut self, path: &'ast Path) {
        self.record_qualified_path(path);
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.record_qualified_path(&mac.path);
        self.scan_token_stream(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        if self.exclude_cfg_test && is_cfg_attr_test(attr) {
            return;
        }
        self.record_qualified_path(attr.path());
        if let Meta::List(list) = &attr.meta {
            self.scan_token_stream(list.tokens.clone());
        }
        visit::visit_attribute(self, attr);
    }
}

fn is_ignored_path_head(head: &str) -> bool {
    matches!(
        head,
        "crate"
            | "self"
            | "super"
            | "std"
            | "core"
            | "alloc"
            | "proc_macro"
            | "test"
            | "clippy"
            | "rustfmt"
    )
}

fn is_double_colon(tokens: &[TokenTree], start: usize) -> bool {
    matches!(
        (tokens.get(start), tokens.get(start + 1)),
        (Some(TokenTree::Punct(first)), Some(TokenTree::Punct(second)))
            if first.as_char() == ':' && first.spacing() == Spacing::Joint && second.as_char() == ':'
    )
}

fn is_path_continuation(tokens: &[TokenTree], idx: usize) -> bool {
    if idx < 3 || !is_double_colon(tokens, idx - 2) {
        return false;
    }
    match tokens.get(idx - 3) {
        Some(TokenTree::Ident(_)) => true,
        Some(TokenTree::Punct(punct)) => punct.as_char() == '>',
        _ => false,
    }
}

fn expr_attrs(expr: &Expr) -> &[Attribute] {
    match_expr_attrs!(
        expr,
        [
            Array, Assign, Async, Await, Binary, Block, Break, Call, Cast, Closure, Const,
            Continue, Field, ForLoop, Group, If, Index, Infer, Let, Lit, Loop, Macro, Match,
            MethodCall, Paren, Path, Range, RawAddr, Reference, Repeat, Return, Struct, Try,
            TryBlock, Tuple, Unary, Unsafe, While, Yield,
        ]
    )
}

fn mod_path_attr(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        let Meta::NameValue(nv) = &attr.meta else {
            continue;
        };
        if !nv.path.is_ident("path") {
            continue;
        }
        if let Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) = &nv.value
        {
            let value = lit.value();
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn impl_item_attrs(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(i) => &i.attrs,
        ImplItem::Fn(i) => &i.attrs,
        ImplItem::Type(i) => &i.attrs,
        ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(i) => &i.attrs,
        TraitItem::Fn(i) => &i.attrs,
        TraitItem::Type(i) => &i.attrs,
        TraitItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn collect_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    allow_local_filter: bool,
    local_idents: &HashSet<String>,
    out: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, allow_local_filter, local_idents, out);
            prefix.pop();
        }
        UseTree::Name(name) => record_use_ident(
            &name.ident.to_string(),
            None,
            prefix,
            allow_local_filter,
            local_idents,
            out,
        ),
        UseTree::Rename(rename) => record_use_ident(
            &rename.ident.to_string(),
            Some(&rename.rename.to_string()),
            prefix,
            allow_local_filter,
            local_idents,
            out,
        ),
        UseTree::Glob(_) => {
            if !prefix.is_empty() && !is_local_root(&prefix[0], allow_local_filter, local_idents) {
                out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, prefix, allow_local_filter, local_idents, out);
            }
        }
    }
}

fn record_use_ident(
    ident: &str,
    alias: Option<&str>,
    prefix: &mut Vec<String>,
    allow_local_filter: bool,
    local_idents: &HashSet<String>,
    out: &mut BTreeSet<String>,
) {
    if ident == "self" {
        if !prefix.is_empty() && !is_local_root(&prefix[0], allow_local_filter, local_idents) {
            let path = prefix.join("::");
            let leaf = prefix.last().map(String::as_str);
            out.insert(format_use_specifier(&path, leaf, alias));
        }
    } else {
        let root = prefix.first().map_or(ident, String::as_str);
        if is_local_root(root, allow_local_filter, local_idents) {
            return;
        }
        prefix.push(ident.to_owned());
        let path = prefix.join("::");
        out.insert(format_use_specifier(&path, Some(ident), alias));
        prefix.pop();
    }
}

fn format_use_specifier(path: &str, leaf: Option<&str>, alias: Option<&str>) -> String {
    match alias {
        Some(rename) if Some(rename) != leaf => format!("{USE_PREFIX}{path} as {rename}"),
        _ => format!("{USE_PREFIX}{path}"),
    }
}

fn is_local_root(root: &str, allow_local_filter: bool, local_idents: &HashSet<String>) -> bool {
    allow_local_filter && !matches!(root, "crate" | "self" | "super") && local_idents.contains(root)
}
