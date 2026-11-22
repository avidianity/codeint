use std::collections::BTreeSet;

use syn::visit::{self, Visit};
use syn::{File, Item, ItemMod, ItemUse, UseTree};

use crate::cfg_test::{has_cfg_test, item_attrs};

pub(crate) const MOD_PREFIX: &str = "mod:";
pub(crate) const USE_PREFIX: &str = "use:";

pub(crate) fn runtime_import_specifiers(file: &File, exclude_cfg_test: bool) -> Vec<String> {
    let mut collector = ImportCollector {
        exclude_cfg_test,
        specifiers: BTreeSet::new(),
    };
    collector.visit_file(file);
    collector.specifiers.into_iter().collect()
}

struct ImportCollector {
    exclude_cfg_test: bool,
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
        let mut prefix = Vec::new();
        collect_use_tree(&node.tree, &mut prefix, &mut self.specifiers);
    }
}

fn collect_use_tree(tree: &UseTree, prefix: &mut Vec<String>, out: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => record_use_ident(&name.ident.to_string(), prefix, out),
        UseTree::Rename(rename) => record_use_ident(&rename.ident.to_string(), prefix, out),
        UseTree::Glob(_) => {
            if !prefix.is_empty() {
                out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, prefix, out);
            }
        }
    }
}

fn record_use_ident(ident: &str, prefix: &mut Vec<String>, out: &mut BTreeSet<String>) {
    if ident == "self" {
        if !prefix.is_empty() {
            out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
        }
    } else {
        prefix.push(ident.to_owned());
        out.insert(format!("{USE_PREFIX}{}", prefix.join("::")));
        prefix.pop();
    }
}
