use super::RefMap;
use crate::AppError;

impl RefMap {
    /// Copies the entry `root` and its ancestor chain from `source` into this map and points
    /// every entry scoped to `root` at the copy, so scoped refs resolve inside this map.
    pub(crate) fn adopt_root_chain(&mut self, source: &RefMap, root: &str) -> Result<(), AppError> {
        let scoped: Vec<String> = self
            .inner
            .iter()
            .filter(|(_, entry)| entry.scope.root_ref.as_deref() == Some(root))
            .map(|(ref_id, _)| ref_id.clone())
            .collect();
        let mut chain = vec![root.to_owned()];
        while let Some(parent) = source
            .get(chain.last().map_or("", String::as_str))
            .and_then(|entry| entry.scope.root_ref.clone())
        {
            if chain.contains(&parent) {
                return Err(AppError::invalid_input("Scoped ref contains a root cycle"));
            }
            chain.push(parent);
        }
        let mut copied: Option<String> = None;
        for ref_id in chain.iter().rev() {
            let mut entry = source.get(ref_id).cloned().ok_or_else(|| {
                AppError::invalid_input("Scoped ref's root is absent from its source snapshot")
            })?;
            entry.scope.root_ref = copied.take();
            copied = Some(self.try_allocate(entry)?);
        }
        for ref_id in scoped {
            if let Some(entry) = self.inner.get_mut(&ref_id) {
                entry.scope.root_ref.clone_from(&copied);
            }
        }
        Ok(())
    }
}
