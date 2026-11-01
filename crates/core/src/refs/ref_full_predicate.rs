use crate::{AccessibilityNode, ElementIdentifier, IdentifierKind};

/// The identity fields a full predicate compares: role plus whichever of native
/// identifier, stable name and description an element records. Mutable values
/// and bounds are never identity.
#[derive(Clone, Copy)]
pub(crate) struct FullPredicate<'a> {
    pub(crate) role: &'a str,
    pub(crate) native_id: Option<(IdentifierKind, &'a str)>,
    pub(crate) name: Option<&'a str>,
    pub(crate) description: Option<&'a str>,
}

impl<'a> FullPredicate<'a> {
    pub(crate) fn of_node(node: &'a AccessibilityNode) -> Self {
        Self::of_fields(
            &node.role,
            node.identity.name.as_deref(),
            node.identity.value.as_deref(),
            node.identity.description.as_deref(),
            node.identity.native_id.as_ref(),
        )
    }

    fn of_fields(
        role: &'a str,
        name: Option<&'a str>,
        value: Option<&'a str>,
        description: Option<&'a str>,
        native_id: Option<&'a ElementIdentifier>,
    ) -> Self {
        Self {
            role,
            native_id: native_id.and_then(|identifier| {
                crate::tree::accname::non_blank(Some(identifier.value.as_str()))
                    .map(|value| (identifier.kind, value))
            }),
            name: super::ref_identity::stable_name(role, name, value),
            description: crate::tree::accname::non_blank(description),
        }
    }
}
