use codeint_config::{Codeowners, OwnerFilter};

#[derive(Clone, Copy, Default)]
pub struct OwnershipOptions<'a> {
    pub codeowners: Option<&'a Codeowners>,
    pub owner_filter: Option<&'a OwnerFilter>,
}

pub(crate) trait OwnedFinding {
    fn assign_owners(&mut self, codeowners: &Codeowners);
    fn owners(&self) -> &[String];
}

impl OwnershipOptions<'_> {
    pub(crate) fn apply<T: OwnedFinding>(&self, mut items: Vec<T>) -> Vec<T> {
        if let Some(codeowners) = self.codeowners {
            for item in &mut items {
                item.assign_owners(codeowners);
            }
        }
        if let Some(filter) = self.owner_filter {
            items.retain(|item| filter.matches_any(item.owners()));
        }
        items
    }
}
