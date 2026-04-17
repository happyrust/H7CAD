// ATTSYNC tool — ribbon definition + attribute synchronisation helpers.
//
// ATTSYNC reshapes the per-INSERT `attributes` vector to match the set of
// `AttributeDefinition` entities that live inside a block record.  Existing
// values for tags that still exist in the block definition are preserved;
// stale tags are dropped; newly defined tags are appended using the attdef
// default value.

use acadrust::entities::{AttributeDefinition, AttributeEntity};

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/attsync.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "ATTSYNC",
        label: "Synchronize",
        icon: ICON,
        event: ModuleEvent::Command("ATTSYNC".to_string()),
    }
}

/// Result of a single `sync_insert_attributes` call — for reporting.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct SyncDelta {
    /// Number of attributes appended (tag present in block, missing on INSERT).
    pub added: usize,
    /// Number of attributes removed (tag on INSERT, not present in block).
    pub removed: usize,
    /// Number of attributes whose value was preserved across the sync.
    pub preserved: usize,
}

/// Reshape `existing` so its tag list matches `attdefs` in order.
///
/// Returns the new attributes vector (in the order dictated by `attdefs`) and
/// a delta summary.  Existing values for tags that remain are kept; dropped
/// tags are not recoverable; new tags are populated via
/// `AttributeEntity::from_definition(attdef, None)` (= attdef default_value).
pub fn sync_insert_attributes(
    attdefs: &[AttributeDefinition],
    existing: &[AttributeEntity],
) -> (Vec<AttributeEntity>, SyncDelta) {
    let mut delta = SyncDelta::default();
    let mut out: Vec<AttributeEntity> = Vec::with_capacity(attdefs.len());

    // Use case-sensitive tag comparison — DXF tags are conventionally
    // upper-case but the spec treats them as literal strings; preserving
    // existing behaviour rather than silently folding case.
    for attdef in attdefs {
        if let Some(prev) = existing.iter().find(|a| a.tag == attdef.tag) {
            // Preserve the prior value; all other metadata comes from the
            // attdef so the INSERT stays geometrically consistent with its
            // block definition.
            let mut fresh = AttributeEntity::from_definition(attdef, Some(prev.value.clone()));
            // Copy over properties that the user can sensibly override on a
            // per-instance basis (handle stays zero — the host allocates).
            fresh.common.handle = prev.common.handle;
            out.push(fresh);
            delta.preserved += 1;
        } else {
            out.push(AttributeEntity::from_definition(attdef, None));
            delta.added += 1;
        }
    }

    // Tags present on the INSERT but no longer defined in the block are
    // implicitly dropped — count them for the report.
    let kept_tags: std::collections::HashSet<&str> =
        attdefs.iter().map(|a| a.tag.as_str()).collect();
    delta.removed = existing
        .iter()
        .filter(|a| !kept_tags.contains(a.tag.as_str()))
        .count();

    (out, delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::entities::{AttributeDefinition, AttributeEntity};

    fn attdef(tag: &str, default: &str) -> AttributeDefinition {
        AttributeDefinition::new(tag.to_string(), tag.to_string(), default.to_string())
    }

    fn existing_attr(tag: &str, value: &str) -> AttributeEntity {
        AttributeEntity::new(tag.to_string(), value.to_string())
    }

    #[test]
    fn adds_missing_tags_from_definition_default() {
        let defs = vec![attdef("SIZE", "10"), attdef("COLOR", "red")];
        let existing = vec![];
        let (out, d) = sync_insert_attributes(&defs, &existing);
        assert_eq!(d.added, 2);
        assert_eq!(d.removed, 0);
        assert_eq!(d.preserved, 0);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].tag, "SIZE");
        assert_eq!(out[0].value, "10");
        assert_eq!(out[1].tag, "COLOR");
        assert_eq!(out[1].value, "red");
    }

    #[test]
    fn preserves_existing_values_for_matching_tags() {
        let defs = vec![attdef("SIZE", "10"), attdef("COLOR", "red")];
        let existing = vec![
            existing_attr("SIZE", "42"),
            existing_attr("COLOR", "blue"),
        ];
        let (out, d) = sync_insert_attributes(&defs, &existing);
        assert_eq!(d.added, 0);
        assert_eq!(d.removed, 0);
        assert_eq!(d.preserved, 2);
        assert_eq!(out[0].value, "42", "preserved user-entered SIZE");
        assert_eq!(out[1].value, "blue", "preserved user-entered COLOR");
    }

    #[test]
    fn drops_stale_tags_not_in_definition() {
        let defs = vec![attdef("SIZE", "10")];
        let existing = vec![
            existing_attr("SIZE", "42"),
            existing_attr("OBSOLETE", "gone"),
        ];
        let (out, d) = sync_insert_attributes(&defs, &existing);
        assert_eq!(d.added, 0);
        assert_eq!(d.removed, 1);
        assert_eq!(d.preserved, 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tag, "SIZE");
        assert_eq!(out[0].value, "42");
    }

    #[test]
    fn mixed_add_remove_preserve() {
        let defs = vec![
            attdef("A", "a-default"),
            attdef("B", "b-default"),
            attdef("C", "c-default"),
        ];
        let existing = vec![
            existing_attr("B", "b-user"),
            existing_attr("X", "stale"),
        ];
        let (out, d) = sync_insert_attributes(&defs, &existing);
        assert_eq!(d.added, 2, "A and C are new");
        assert_eq!(d.removed, 1, "X is dropped");
        assert_eq!(d.preserved, 1, "B kept its user value");
        assert_eq!(
            out.iter().map(|a| a.tag.as_str()).collect::<Vec<_>>(),
            vec!["A", "B", "C"],
            "output order matches attdef order"
        );
        assert_eq!(out[1].value, "b-user");
    }

    #[test]
    fn empty_defs_removes_all() {
        let defs: Vec<AttributeDefinition> = vec![];
        let existing = vec![existing_attr("A", "1"), existing_attr("B", "2")];
        let (out, d) = sync_insert_attributes(&defs, &existing);
        assert!(out.is_empty());
        assert_eq!(d.removed, 2);
        assert_eq!(d.added, 0);
        assert_eq!(d.preserved, 0);
    }
}
