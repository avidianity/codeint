use std::collections::{BTreeSet, HashSet};

use syn::visit::{self, Visit};
use syn::{File, Item, ItemExternCrate, ItemMacro, ItemMod, ItemUse, UseTree};

use crate::cfg_test::{has_cfg_test, item_attrs};

pub(crate) const MOD_PREFIX: &str = "mod:";
pub(crate) const USE_PREFIX: &str = "use:";

pub(crate) fn runtime_import_specifiers(file: &File, exclude_cfg_test: bool) -> Vec<String> {
    let mut local_collector = LocalIdentCollector {
        exclude_cfg_test,
        idents: HashSet::new(),
    };
    local_collector.visit_file(file);
    let mut collector = ImportCollector {
        exclude_cfg_test,
        local_idents: local_collector.idents,
        specifiers: BTreeSet::new(),
    };
    collector.visit_file(file);
    collector.specifiers.into_iter().collect()
}

struct LocalIdentCollector {
    exclude_cfg_test: bool,
    idents: HashSet<String>,
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

fn is_bindgen_macro(path: &syn::Path) -> bool {
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
    specifiers: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ImportCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.exclude_cfg_test && has_cfg_test(item_attrs(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if node.content.is_some() {
            visit::visit_item_mod(self, node);
        }
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
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
            prefix,
            allow_local_filter,
            local_idents,
            out,
        ),
        UseTree::Rename(rename) => record_use_ident(
            &rename.ident.to_string(),
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
    prefix: &mut Vec<String>,
    allow_local_filter: bool,
    local_idents: &HashSet<String>,
    out: &mut BTreeSet<String>,
) {
    if ident == "self" {
        if !prefix.is_empty() && !is_local_root(&prefix[0], allow_local_filter, local_idents) {
            out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
        }
    } else {
        let root = prefix.first().map_or(ident, String::as_str);
        if is_local_root(root, allow_local_filter, local_idents) {
            return;
        }
        prefix.push(ident.to_owned());
        out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
        prefix.pop();
    }
}

fn is_local_root(root: &str, allow_local_filter: bool, local_idents: &HashSet<String>) -> bool {
    allow_local_filter && !matches!(root, "crate" | "self" | "super") && local_idents.contains(root)
}
