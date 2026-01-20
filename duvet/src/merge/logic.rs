// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Core merge logic for combining multiple JSON reports.

use super::schema::{JsonAnnotation, JsonReport};
use std::collections::{BTreeMap, HashMap};

/// A stable key for identifying annotations across multiple reports.
///
/// Annotation IDs in each report are just array indices (0, 1, 2, ...),
/// so they collide when merging. This key is based on annotation content
/// to uniquely identify annotations across all reports.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnnotationKey {
    /// Target specification path
    pub target_path: String,
    
    /// Optional target section within the specification
    pub target_section: Option<String>,
    
    /// Source file path containing the annotation
    pub source: String,
    
    /// Line number in the source file (0 if not specified)
    pub line: usize,
}

impl From<&JsonAnnotation> for AnnotationKey {
    fn from(anno: &JsonAnnotation) -> Self {
        Self {
            target_path: anno.target_path.clone(),
            target_section: anno.target_section.clone(),
            source: anno.source.clone(),
            line: anno.line.unwrap_or(0),
        }
    }
}

/// Result of collecting and deduplicating annotations from multiple reports.
#[derive(Debug)]
pub struct AnnotationCollection {
    /// Deduplicated annotations, keyed by their content-based key
    pub annotation_map: BTreeMap<AnnotationKey, JsonAnnotation>,
    
    /// Mapping from (report_index, old_annotation_id) to annotation key
    /// Used for remapping annotation IDs in statuses
    pub old_to_key: HashMap<(usize, usize), AnnotationKey>,
}

impl AnnotationCollection {
    /// Create a new empty annotation collection
    pub fn new() -> Self {
        Self {
            annotation_map: BTreeMap::new(),
            old_to_key: HashMap::new(),
        }
    }
}

/// Result of assigning new sequential IDs to annotations.
#[derive(Debug)]
pub struct IdAssignment {
    /// Merged annotations in their new order with sequential IDs
    pub merged_annotations: Vec<JsonAnnotation>,
    
    /// Mapping from annotation key to new sequential ID
    pub key_to_new_id: HashMap<AnnotationKey, usize>,
}

impl IdAssignment {
    /// Create a new empty ID assignment
    pub fn new() -> Self {
        Self {
            merged_annotations: Vec::new(),
            key_to_new_id: HashMap::new(),
        }
    }
}

/// Collect and deduplicate annotations from multiple reports.
///
/// This function:
/// 1. Iterates through all annotations in all reports
/// 2. Creates a content-based key for each annotation
/// 3. Deduplicates annotations with the same key (keeps first occurrence)
/// 4. Tracks the mapping from (report_index, old_id) to annotation key
///
/// # Arguments
/// * `reports` - Slice of JSON reports to process
///
/// # Returns
/// An `AnnotationCollection` containing deduplicated annotations and ID mappings
pub fn collect_annotations(reports: &[JsonReport]) -> AnnotationCollection {
    let mut collection = AnnotationCollection::new();
    
    for (report_index, report) in reports.iter().enumerate() {
        for (old_id, annotation) in report.annotations.iter().enumerate() {
            let key = AnnotationKey::from(annotation);
            
            // Add to annotation map if not already present (deduplication)
            collection.annotation_map.entry(key.clone())
                .or_insert_with(|| annotation.clone());
            
            // Track the mapping from old ID to key
            collection.old_to_key.insert((report_index, old_id), key);
        }
    }
    
    collection
}

/// Assign new sequential IDs to annotations based on their sorted keys.
///
/// This function:
/// 1. Iterates through annotations in sorted order (by AnnotationKey)
/// 2. Assigns sequential IDs starting from 0
/// 3. Builds a mapping from annotation key to new ID
/// 4. Creates a vector of merged annotations in the new order
///
/// # Arguments
/// * `collection` - The annotation collection with deduplicated annotations
///
/// # Returns
/// An `IdAssignment` containing the merged annotations vector and key-to-ID mapping
pub fn assign_new_ids(collection: &AnnotationCollection) -> IdAssignment {
    let mut assignment = IdAssignment::new();
    
    // BTreeMap iteration is in sorted order by key
    for (key, annotation) in &collection.annotation_map {
        let new_id = assignment.merged_annotations.len();
        assignment.merged_annotations.push(annotation.clone());
        assignment.key_to_new_id.insert(key.clone(), new_id);
    }
    
    assignment
}

/// Result of merging statuses from multiple reports.
#[derive(Debug)]
pub struct StatusCollection {
    /// Merged statuses, keyed by annotation key
    /// Status counts are summed for annotations with the same key
    pub status_by_key: HashMap<AnnotationKey, super::schema::JsonStatus>,
}

impl StatusCollection {
    /// Create a new empty status collection
    pub fn new() -> Self {
        Self {
            status_by_key: HashMap::new(),
        }
    }
}

/// Merge statuses from multiple reports using annotation keys.
///
/// This function:
/// 1. Iterates through all statuses in all reports
/// 2. Maps old annotation IDs to annotation keys using the collection
/// 3. Merges status counts for annotations with the same key
/// 4. Collects related annotation IDs (before remapping)
///
/// # Arguments
/// * `reports` - Slice of JSON reports to process
/// * `collection` - The annotation collection with ID-to-key mappings
///
/// # Returns
/// A `StatusCollection` containing merged statuses keyed by annotation key
pub fn merge_statuses(
    reports: &[JsonReport],
    collection: &AnnotationCollection,
) -> StatusCollection {
    let mut status_collection = StatusCollection::new();
    
    for (report_index, report) in reports.iter().enumerate() {
        for (old_id_str, status) in &report.statuses {
            // Parse the old annotation ID from the string key
            let old_id = match old_id_str.parse::<usize>() {
                Ok(id) => id,
                Err(_) => continue, // Skip invalid IDs
            };
            
            // Look up the annotation key for this old ID
            let key = match collection.old_to_key.get(&(report_index, old_id)) {
                Some(k) => k.clone(),
                None => continue, // Skip if annotation not found
            };
            
            // Merge the status into the collection
            status_collection.status_by_key
                .entry(key)
                .and_modify(|existing| {
                    // Add status counts
                    add_status_counts(existing, status);
                })
                .or_insert_with(|| status.clone());
        }
    }
    
    status_collection
}

/// Add status counts from one status to another.
///
/// This helper function adds all count fields (spec, incomplete, citation, etc.)
/// and unions the related IDs arrays.
///
/// # Arguments
/// * `target` - The status to add counts to (modified in place)
/// * `source` - The status to add counts from
fn add_status_counts(
    target: &mut super::schema::JsonStatus,
    source: &super::schema::JsonStatus,
) {
    // Add each count field
    target.spec = Some(target.spec.unwrap_or(0) + source.spec.unwrap_or(0));
    target.incomplete = Some(target.incomplete.unwrap_or(0) + source.incomplete.unwrap_or(0));
    target.citation = Some(target.citation.unwrap_or(0) + source.citation.unwrap_or(0));
    target.implication = Some(target.implication.unwrap_or(0) + source.implication.unwrap_or(0));
    target.test = Some(target.test.unwrap_or(0) + source.test.unwrap_or(0));
    target.exception = Some(target.exception.unwrap_or(0) + source.exception.unwrap_or(0));
    target.todo = Some(target.todo.unwrap_or(0) + source.todo.unwrap_or(0));
    
    // Union related IDs (will be remapped later)
    if let Some(source_related) = &source.related {
        let target_related = target.related.get_or_insert_with(Vec::new);
        target_related.extend(source_related.iter().copied());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::schema::JsonReport;

    fn create_test_annotation(
        source: &str,
        target_path: &str,
        target_section: Option<&str>,
        line: Option<usize>,
    ) -> JsonAnnotation {
        JsonAnnotation {
            source: source.to_string(),
            target_path: target_path.to_string(),
            target_section: target_section.map(|s| s.to_string()),
            line,
            anno_type: None,
            level: None,
            comment: None,
            feature: None,
            tracking_issue: None,
            tags: None,
        }
    }

    fn create_test_status(
        spec: Option<usize>,
        incomplete: Option<usize>,
        citation: Option<usize>,
        test: Option<usize>,
        related: Option<Vec<usize>>,
    ) -> crate::merge::schema::JsonStatus {
        crate::merge::schema::JsonStatus {
            spec,
            incomplete,
            citation,
            implication: None,
            test,
            exception: None,
            todo: None,
            related,
        }
    }

    #[test]
    fn test_annotation_key_generation() {
        let anno = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let key = AnnotationKey::from(&anno);
        
        assert_eq!(key.source, "src/lib.rs");
        assert_eq!(key.target_path, "https://example.com/spec");
        assert_eq!(key.target_section, Some("section-1".to_string()));
        assert_eq!(key.line, 42);
    }

    #[test]
    fn test_annotation_key_without_line() {
        let anno = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            None,
        );
        
        let key = AnnotationKey::from(&anno);
        
        assert_eq!(key.line, 0);
    }

    #[test]
    fn test_annotation_key_without_section() {
        let anno = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            None,
            Some(42),
        );
        
        let key = AnnotationKey::from(&anno);
        
        assert!(key.target_section.is_none());
    }

    #[test]
    fn test_annotation_key_equality_same_content() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_annotation_key_inequality_different_source() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/main.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_annotation_key_inequality_different_line() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(43),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_annotation_key_inequality_different_section() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-2"),
            Some(42),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_annotation_key_inequality_different_target_path() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec1",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec2",
            Some("section-1"),
            Some(42),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_annotation_key_ordering() {
        let anno1 = create_test_annotation(
            "src/a.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let anno2 = create_test_annotation(
            "src/b.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        // Keys should be ordered by target_path first, then target_section, then source, then line
        assert!(key1 < key2);
    }

    #[test]
    fn test_annotation_key_ordering_by_line() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(20),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert!(key1 < key2);
    }

    #[test]
    fn test_annotation_key_ordering_by_section() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-2"),
            Some(10),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        assert!(key1 < key2);
    }

    #[test]
    fn test_annotation_key_ordering_none_vs_some_section() {
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            None,
            Some(10),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        // None should come before Some
        assert!(key1 < key2);
    }

    #[test]
    fn test_annotation_key_hash_consistency() {
        use std::collections::HashSet;
        
        let anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        let mut set = HashSet::new();
        set.insert(key1);
        
        // key2 should be considered equal and not inserted again
        assert!(set.contains(&key2));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_annotation_key_in_btreemap() {
        use std::collections::BTreeMap;
        
        let anno1 = create_test_annotation(
            "src/a.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let anno2 = create_test_annotation(
            "src/b.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let anno3 = create_test_annotation(
            "src/c.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(10),
        );
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        let key3 = AnnotationKey::from(&anno3);
        
        let mut map = BTreeMap::new();
        map.insert(key2.clone(), "second");
        map.insert(key1.clone(), "first");
        map.insert(key3.clone(), "third");
        
        // BTreeMap should maintain sorted order
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], &key1);
        assert_eq!(keys[1], &key2);
        assert_eq!(keys[2], &key3);
    }

    #[test]
    fn test_annotation_key_ignores_other_fields() {
        // Keys should be equal even if other annotation fields differ
        let mut anno1 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        anno1.anno_type = Some("SPEC".to_string());
        anno1.level = Some("MUST".to_string());
        anno1.comment = Some("Comment 1".to_string());
        
        let mut anno2 = create_test_annotation(
            "src/lib.rs",
            "https://example.com/spec",
            Some("section-1"),
            Some(42),
        );
        anno2.anno_type = Some("TEST".to_string());
        anno2.level = Some("SHOULD".to_string());
        anno2.comment = Some("Comment 2".to_string());
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        // Keys should be equal because they have the same location
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_collect_annotations_empty_reports() {
        let reports = vec![];
        let collection = collect_annotations(&reports);
        
        assert_eq!(collection.annotation_map.len(), 0);
        assert_eq!(collection.old_to_key.len(), 0);
    }

    #[test]
    fn test_collect_annotations_single_report() {
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10)),
                create_test_annotation("src/lib.rs", "spec1", Some("s2"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report]);
        
        assert_eq!(collection.annotation_map.len(), 2);
        assert_eq!(collection.old_to_key.len(), 2);
        
        // Verify old_to_key mappings
        assert!(collection.old_to_key.contains_key(&(0, 0)));
        assert!(collection.old_to_key.contains_key(&(0, 1)));
    }

    #[test]
    fn test_collect_annotations_multiple_reports_disjoint() {
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Both annotations should be present (different keys)
        assert_eq!(collection.annotation_map.len(), 2);
        assert_eq!(collection.old_to_key.len(), 2);
        
        // Verify mappings for both reports
        assert!(collection.old_to_key.contains_key(&(0, 0)));
        assert!(collection.old_to_key.contains_key(&(1, 0)));
    }

    #[test]
    fn test_collect_annotations_deduplication() {
        // Create two reports with the same annotation (same key)
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Should only have 1 annotation in the map (deduplicated)
        assert_eq!(collection.annotation_map.len(), 1);
        
        // But should have 2 entries in old_to_key (one for each report)
        assert_eq!(collection.old_to_key.len(), 2);
        
        // Both should map to the same key
        let key1 = collection.old_to_key.get(&(0, 0)).unwrap();
        let key2 = collection.old_to_key.get(&(1, 0)).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_collect_annotations_partial_overlap() {
        let shared_anno = create_test_annotation("src/shared.rs", "spec1", Some("s1"), Some(10));
        let unique_anno1 = create_test_annotation("src/unique1.rs", "spec1", Some("s1"), Some(20));
        let unique_anno2 = create_test_annotation("src/unique2.rs", "spec1", Some("s1"), Some(30));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno.clone(), unique_anno1],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno, unique_anno2],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Should have 3 unique annotations (1 shared + 2 unique)
        assert_eq!(collection.annotation_map.len(), 3);
        
        // Should have 4 old_to_key entries (2 annotations per report)
        assert_eq!(collection.old_to_key.len(), 4);
    }

    #[test]
    fn test_collect_annotations_preserves_first_occurrence() {
        // Create two annotations with same key but different metadata
        let mut anno1 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        anno1.comment = Some("First comment".to_string());
        anno1.anno_type = Some("SPEC".to_string());
        
        let mut anno2 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        anno2.comment = Some("Second comment".to_string());
        anno2.anno_type = Some("TEST".to_string());
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Should only have 1 annotation
        assert_eq!(collection.annotation_map.len(), 1);
        
        // Should preserve the first occurrence (from report1)
        let key = AnnotationKey::from(&anno1);
        let stored_anno = collection.annotation_map.get(&key).unwrap();
        assert_eq!(stored_anno.comment, Some("First comment".to_string()));
        assert_eq!(stored_anno.anno_type, Some("SPEC".to_string()));
    }

    #[test]
    fn test_collect_annotations_maintains_sorted_order() {
        // Create annotations that would be out of order if not sorted
        let anno1 = create_test_annotation("src/z.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno3 = create_test_annotation("src/m.rs", "spec1", Some("s1"), Some(10));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report]);
        
        // BTreeMap should maintain sorted order
        let keys: Vec<_> = collection.annotation_map.keys().collect();
        assert_eq!(keys.len(), 3);
        
        // Verify sorted by source field (a < m < z)
        assert_eq!(keys[0].source, "src/a.rs");
        assert_eq!(keys[1].source, "src/m.rs");
        assert_eq!(keys[2].source, "src/z.rs");
    }

    #[test]
    fn test_collect_annotations_multiple_reports_with_different_indices() {
        // Test that old_to_key correctly tracks report indices
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/d.rs", "spec1", Some("s1"), Some(40)),
                create_test_annotation("src/e.rs", "spec1", Some("s1"), Some(50)),
                create_test_annotation("src/f.rs", "spec1", Some("s1"), Some(60)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2, report3]);
        
        // Should have 6 unique annotations
        assert_eq!(collection.annotation_map.len(), 6);
        
        // Should have 6 old_to_key entries
        assert_eq!(collection.old_to_key.len(), 6);
        
        // Verify specific mappings
        assert!(collection.old_to_key.contains_key(&(0, 0))); // report1, anno 0
        assert!(collection.old_to_key.contains_key(&(0, 1))); // report1, anno 1
        assert!(collection.old_to_key.contains_key(&(1, 0))); // report2, anno 0
        assert!(collection.old_to_key.contains_key(&(2, 0))); // report3, anno 0
        assert!(collection.old_to_key.contains_key(&(2, 1))); // report3, anno 1
        assert!(collection.old_to_key.contains_key(&(2, 2))); // report3, anno 2
    }

    #[test]
    fn test_collect_annotations_empty_report_in_middle() {
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![], // Empty
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2, report3]);
        
        // Should have 2 annotations (report2 is empty)
        assert_eq!(collection.annotation_map.len(), 2);
        
        // Should have 2 old_to_key entries
        assert_eq!(collection.old_to_key.len(), 2);
        
        // Verify correct report indices
        assert!(collection.old_to_key.contains_key(&(0, 0)));
        assert!(collection.old_to_key.contains_key(&(2, 0)));
        assert!(!collection.old_to_key.contains_key(&(1, 0))); // report2 has no annotations
    }

    #[test]
    fn test_id_tracking_correct_report_index() {
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Verify report indices are correct
        let key1 = collection.old_to_key.get(&(0, 0)).unwrap();
        let key2 = collection.old_to_key.get(&(1, 0)).unwrap();
        
        assert_eq!(key1.source, "src/a.rs");
        assert_eq!(key2.source, "src/b.rs");
    }

    #[test]
    fn test_id_tracking_correct_annotation_index() {
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
                create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report]);
        
        // Verify annotation indices are correct
        let key0 = collection.old_to_key.get(&(0, 0)).unwrap();
        let key1 = collection.old_to_key.get(&(0, 1)).unwrap();
        let key2 = collection.old_to_key.get(&(0, 2)).unwrap();
        
        assert_eq!(key0.source, "src/a.rs");
        assert_eq!(key1.source, "src/b.rs");
        assert_eq!(key2.source, "src/c.rs");
    }

    #[test]
    fn test_id_tracking_with_deduplication() {
        // Create same annotation in multiple reports
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2, report3]);
        
        // All three should map to the same key
        let key1 = collection.old_to_key.get(&(0, 0)).unwrap();
        let key2 = collection.old_to_key.get(&(1, 0)).unwrap();
        let key3 = collection.old_to_key.get(&(2, 0)).unwrap();
        
        assert_eq!(key1, key2);
        assert_eq!(key2, key3);
        
        // But only one entry in annotation_map
        assert_eq!(collection.annotation_map.len(), 1);
        assert!(collection.annotation_map.contains_key(key1));
    }

    #[test]
    fn test_id_tracking_lookup_by_old_id() {
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10)),
                create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        
        // Test that we can look up keys by (report_index, old_id)
        let key_r0_a0 = collection.old_to_key.get(&(0, 0));
        let key_r0_a1 = collection.old_to_key.get(&(0, 1));
        let key_r1_a0 = collection.old_to_key.get(&(1, 0));
        let key_r1_a1 = collection.old_to_key.get(&(1, 1)); // Doesn't exist
        
        assert!(key_r0_a0.is_some());
        assert!(key_r0_a1.is_some());
        assert!(key_r1_a0.is_some());
        assert!(key_r1_a1.is_none());
    }

    #[test]
    fn test_id_tracking_all_reports_same_annotation() {
        // Edge case: all reports have the exact same single annotation
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let reports: Vec<JsonReport> = (0..5)
            .map(|_| JsonReport {
                blob_link: None,
                issue_link: None,
                specifications: HashMap::new(),
                annotations: vec![anno.clone()],
                statuses: HashMap::new(),
                refs: vec![],
            })
            .collect();
        
        let collection = collect_annotations(&reports);
        
        // Should have only 1 unique annotation
        assert_eq!(collection.annotation_map.len(), 1);
        
        // But should have 5 old_to_key entries (one per report)
        assert_eq!(collection.old_to_key.len(), 5);
        
        // All should map to the same key
        let keys: Vec<_> = (0..5)
            .map(|i| collection.old_to_key.get(&(i, 0)).unwrap())
            .collect();
        
        for i in 1..5 {
            assert_eq!(keys[0], keys[i]);
        }
    }

    #[test]
    fn test_assign_new_ids_empty_collection() {
        let collection = AnnotationCollection::new();
        let assignment = assign_new_ids(&collection);
        
        assert_eq!(assignment.merged_annotations.len(), 0);
        assert_eq!(assignment.key_to_new_id.len(), 0);
    }

    #[test]
    fn test_assign_new_ids_single_annotation() {
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut collection = AnnotationCollection::new();
        collection.annotation_map.insert(key.clone(), anno.clone());
        
        let assignment = assign_new_ids(&collection);
        
        assert_eq!(assignment.merged_annotations.len(), 1);
        assert_eq!(assignment.key_to_new_id.len(), 1);
        
        // Should assign ID 0
        assert_eq!(assignment.key_to_new_id.get(&key), Some(&0));
        assert_eq!(assignment.merged_annotations[0].source, "src/lib.rs");
    }

    #[test]
    fn test_assign_new_ids_multiple_annotations() {
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        let key3 = AnnotationKey::from(&anno3);
        
        let mut collection = AnnotationCollection::new();
        collection.annotation_map.insert(key1.clone(), anno1);
        collection.annotation_map.insert(key2.clone(), anno2);
        collection.annotation_map.insert(key3.clone(), anno3);
        
        let assignment = assign_new_ids(&collection);
        
        assert_eq!(assignment.merged_annotations.len(), 3);
        assert_eq!(assignment.key_to_new_id.len(), 3);
        
        // Should assign sequential IDs 0, 1, 2
        assert_eq!(assignment.key_to_new_id.get(&key1), Some(&0));
        assert_eq!(assignment.key_to_new_id.get(&key2), Some(&1));
        assert_eq!(assignment.key_to_new_id.get(&key3), Some(&2));
    }

    #[test]
    fn test_assign_new_ids_preserves_annotation_content() {
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut collection = AnnotationCollection::new();
        collection.annotation_map.insert(key.clone(), anno.clone());
        
        let assignment = assign_new_ids(&collection);
        
        // Verify annotation content is preserved
        assert_eq!(assignment.merged_annotations[0].source, anno.source);
        assert_eq!(assignment.merged_annotations[0].target_path, anno.target_path);
        assert_eq!(assignment.merged_annotations[0].target_section, anno.target_section);
        assert_eq!(assignment.merged_annotations[0].line, anno.line);
    }

    #[test]
    fn test_assign_new_ids_sorted_order() {
        // Create annotations in unsorted order
        let anno_z = create_test_annotation("src/z.rs", "spec1", Some("s1"), Some(10));
        let anno_a = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno_m = create_test_annotation("src/m.rs", "spec1", Some("s1"), Some(10));
        
        let key_z = AnnotationKey::from(&anno_z);
        let key_a = AnnotationKey::from(&anno_a);
        let key_m = AnnotationKey::from(&anno_m);
        
        let mut collection = AnnotationCollection::new();
        // Insert in unsorted order
        collection.annotation_map.insert(key_z.clone(), anno_z);
        collection.annotation_map.insert(key_a.clone(), anno_a);
        collection.annotation_map.insert(key_m.clone(), anno_m);
        
        let assignment = assign_new_ids(&collection);
        
        // BTreeMap should maintain sorted order, so IDs should be assigned in sorted order
        assert_eq!(assignment.merged_annotations[0].source, "src/a.rs");
        assert_eq!(assignment.merged_annotations[1].source, "src/m.rs");
        assert_eq!(assignment.merged_annotations[2].source, "src/z.rs");
        
        // Verify ID mappings match the sorted order
        assert_eq!(assignment.key_to_new_id.get(&key_a), Some(&0));
        assert_eq!(assignment.key_to_new_id.get(&key_m), Some(&1));
        assert_eq!(assignment.key_to_new_id.get(&key_z), Some(&2));
    }

    #[test]
    fn test_assign_new_ids_bidirectional_lookup() {
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        let mut collection = AnnotationCollection::new();
        collection.annotation_map.insert(key1.clone(), anno1.clone());
        collection.annotation_map.insert(key2.clone(), anno2.clone());
        
        let assignment = assign_new_ids(&collection);
        
        // Test bidirectional lookup: key -> ID -> annotation
        let id1 = assignment.key_to_new_id.get(&key1).unwrap();
        let id2 = assignment.key_to_new_id.get(&key2).unwrap();
        
        assert_eq!(assignment.merged_annotations[*id1].source, anno1.source);
        assert_eq!(assignment.merged_annotations[*id2].source, anno2.source);
    }

    #[test]
    fn test_assign_new_ids_from_full_collection() {
        // Test with a collection created by collect_annotations
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/z.rs", "spec1", Some("s1"), Some(10)),
                create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(20)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                create_test_annotation("src/m.rs", "spec1", Some("s1"), Some(30)),
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        let assignment = assign_new_ids(&collection);
        
        // Should have 3 annotations in sorted order
        assert_eq!(assignment.merged_annotations.len(), 3);
        assert_eq!(assignment.merged_annotations[0].source, "src/a.rs");
        assert_eq!(assignment.merged_annotations[1].source, "src/m.rs");
        assert_eq!(assignment.merged_annotations[2].source, "src/z.rs");
        
        // Verify all keys have mappings
        assert_eq!(assignment.key_to_new_id.len(), 3);
    }

    #[test]
    fn test_assign_new_ids_with_deduplication() {
        // Test that deduplicated annotations get single ID
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1, report2]);
        let assignment = assign_new_ids(&collection);
        
        // Should have only 1 annotation (deduplicated)
        assert_eq!(assignment.merged_annotations.len(), 1);
        assert_eq!(assignment.key_to_new_id.len(), 1);
        
        // Both old IDs should map to the same key, which maps to ID 0
        let key = AnnotationKey::from(&anno);
        assert_eq!(assignment.key_to_new_id.get(&key), Some(&0));
    }

    #[test]
    fn test_deterministic_ordering_same_input_order() {
        // Test that the same input produces the same output order
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone(), anno2.clone(), anno3.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        // Run twice with same input
        let collection1 = collect_annotations(&[report.clone()]);
        let assignment1 = assign_new_ids(&collection1);
        
        let collection2 = collect_annotations(&[report]);
        let assignment2 = assign_new_ids(&collection2);
        
        // Should produce identical results
        assert_eq!(assignment1.merged_annotations.len(), assignment2.merged_annotations.len());
        for i in 0..assignment1.merged_annotations.len() {
            assert_eq!(assignment1.merged_annotations[i].source, assignment2.merged_annotations[i].source);
        }
    }

    #[test]
    fn test_deterministic_ordering_different_input_order() {
        // Test that different input order produces the same output order
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let report_ordered = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone(), anno2.clone(), anno3.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report_reversed = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno3.clone(), anno2.clone(), anno1.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection1 = collect_annotations(&[report_ordered]);
        let assignment1 = assign_new_ids(&collection1);
        
        let collection2 = collect_annotations(&[report_reversed]);
        let assignment2 = assign_new_ids(&collection2);
        
        // Should produce identical output order (sorted)
        assert_eq!(assignment1.merged_annotations.len(), 3);
        assert_eq!(assignment2.merged_annotations.len(), 3);
        
        for i in 0..3 {
            assert_eq!(assignment1.merged_annotations[i].source, assignment2.merged_annotations[i].source);
        }
        
        // Both should be in sorted order
        assert_eq!(assignment1.merged_annotations[0].source, "src/a.rs");
        assert_eq!(assignment1.merged_annotations[1].source, "src/b.rs");
        assert_eq!(assignment1.merged_annotations[2].source, "src/c.rs");
    }

    #[test]
    fn test_deterministic_ordering_multiple_reports_different_order() {
        // Test that report order doesn't affect final annotation order
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno3.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        // Try different report orders
        let collection1 = collect_annotations(&[report1.clone(), report2.clone(), report3.clone()]);
        let assignment1 = assign_new_ids(&collection1);
        
        let collection2 = collect_annotations(&[report3.clone(), report1.clone(), report2.clone()]);
        let assignment2 = assign_new_ids(&collection2);
        
        let collection3 = collect_annotations(&[report2, report3, report1]);
        let assignment3 = assign_new_ids(&collection3);
        
        // All should produce the same sorted output
        assert_eq!(assignment1.merged_annotations.len(), 3);
        assert_eq!(assignment2.merged_annotations.len(), 3);
        assert_eq!(assignment3.merged_annotations.len(), 3);
        
        for i in 0..3 {
            assert_eq!(assignment1.merged_annotations[i].source, assignment2.merged_annotations[i].source);
            assert_eq!(assignment2.merged_annotations[i].source, assignment3.merged_annotations[i].source);
        }
    }

    #[test]
    fn test_deterministic_ordering_complex_keys() {
        // Test deterministic ordering with complex keys (different target_path, section, line)
        let anno1 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/lib.rs", "spec1", Some("s2"), Some(10));
        let anno4 = create_test_annotation("src/lib.rs", "spec2", Some("s1"), Some(10));
        
        let report_order1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone(), anno2.clone(), anno3.clone(), anno4.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report_order2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno4.clone(), anno3.clone(), anno2.clone(), anno1.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection1 = collect_annotations(&[report_order1]);
        let assignment1 = assign_new_ids(&collection1);
        
        let collection2 = collect_annotations(&[report_order2]);
        let assignment2 = assign_new_ids(&collection2);
        
        // Should produce identical ordering
        assert_eq!(assignment1.merged_annotations.len(), 4);
        assert_eq!(assignment2.merged_annotations.len(), 4);
        
        for i in 0..4 {
            let a1 = &assignment1.merged_annotations[i];
            let a2 = &assignment2.merged_annotations[i];
            assert_eq!(a1.source, a2.source);
            assert_eq!(a1.target_path, a2.target_path);
            assert_eq!(a1.target_section, a2.target_section);
            assert_eq!(a1.line, a2.line);
        }
    }

    #[test]
    fn test_deterministic_ordering_with_none_values() {
        // Test ordering with None values for section and line
        let anno1 = create_test_annotation("src/lib.rs", "spec1", None, Some(10));
        let anno2 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let anno3 = create_test_annotation("src/lib.rs", "spec1", Some("s1"), None);
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2.clone(), anno3.clone(), anno1.clone()],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report]);
        let assignment = assign_new_ids(&collection);
        
        // Verify deterministic ordering (None < Some for section, 0 < 10 for line)
        assert_eq!(assignment.merged_annotations.len(), 3);
        
        // anno1: section=None, line=10 -> (None, 10)
        // anno2: section=Some("s1"), line=10 -> (Some("s1"), 10)
        // anno3: section=Some("s1"), line=None (0) -> (Some("s1"), 0)
        
        // Expected order: (None, 10) < (Some("s1"), 0) < (Some("s1"), 10)
        assert_eq!(assignment.merged_annotations[0].target_section, None);
        assert_eq!(assignment.merged_annotations[0].line, Some(10));
        
        assert_eq!(assignment.merged_annotations[1].target_section, Some("s1".to_string()));
        assert_eq!(assignment.merged_annotations[1].line, None);
        
        assert_eq!(assignment.merged_annotations[2].target_section, Some("s1".to_string()));
        assert_eq!(assignment.merged_annotations[2].line, Some(10));
    }

    // Status merging tests

    #[test]
    fn test_merge_statuses_empty_reports() {
        let reports = vec![];
        let collection = collect_annotations(&reports);
        let status_collection = merge_statuses(&reports, &collection);
        
        assert_eq!(status_collection.status_by_key.len(), 0);
    }

    #[test]
    fn test_merge_statuses_single_report() {
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            Some(0),
            Some(2),
            Some(1),
            None,
        ));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        assert_eq!(status_collection.status_by_key.len(), 1);
        
        let status = status_collection.status_by_key.get(&key).unwrap();
        assert_eq!(status.spec, Some(1));
        assert_eq!(status.incomplete, Some(0));
        assert_eq!(status.citation, Some(2));
        assert_eq!(status.test, Some(1));
    }

    #[test]
    fn test_merge_statuses_same_key_adds_counts() {
        // Create same annotation in two reports
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(5),
            Some(6),
            Some(7),
            Some(8),
            None,
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        // Should have only one status entry (same key)
        assert_eq!(status_collection.status_by_key.len(), 1);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // Counts should be added
        assert_eq!(merged_status.spec, Some(6));        // 1 + 5
        assert_eq!(merged_status.incomplete, Some(8));  // 2 + 6
        assert_eq!(merged_status.citation, Some(10));   // 3 + 7
        assert_eq!(merged_status.test, Some(12));       // 4 + 8
    }

    #[test]
    fn test_merge_statuses_same_key_with_none_values() {
        // Test that None values are treated as 0
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,  // None should be treated as 0
            Some(3),
            None,
            None,
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            None,
            Some(2),
            Some(4),
            Some(5),
            None,
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // None + Some(x) = Some(x)
        assert_eq!(merged_status.spec, Some(1));        // 1 + 0
        assert_eq!(merged_status.incomplete, Some(2));  // 0 + 2
        assert_eq!(merged_status.citation, Some(7));    // 3 + 4
        assert_eq!(merged_status.test, Some(5));        // 0 + 5
    }

    #[test]
    fn test_merge_statuses_same_key_unions_related_ids() {
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2, 3]),
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![4, 5]),
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // Related IDs should be unioned (not deduplicated yet)
        let related = merged_status.related.as_ref().unwrap();
        assert_eq!(related.len(), 5);
        assert!(related.contains(&1));
        assert!(related.contains(&2));
        assert!(related.contains(&3));
        assert!(related.contains(&4));
        assert!(related.contains(&5));
    }

    #[test]
    fn test_merge_statuses_same_key_preserves_duplicate_related_ids() {
        // Test that duplicate related IDs are preserved (deduplication happens later)
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2]),
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![2, 3]),  // 2 is duplicate
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // Should have 4 entries (including duplicate 2)
        let related = merged_status.related.as_ref().unwrap();
        assert_eq!(related.len(), 4);
        assert_eq!(related, &vec![1, 2, 2, 3]);
    }

    #[test]
    fn test_merge_statuses_same_key_three_reports() {
        // Test merging the same annotation across three reports
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(1), Some(1), None, None, None));
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(2), Some(2), None, None, None));
        
        let mut statuses3 = HashMap::new();
        statuses3.insert("0".to_string(), create_test_status(Some(3), Some(3), None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses2,
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses3,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone(), report3.clone()]);
        let status_collection = merge_statuses(&[report1, report2, report3], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // Should sum all three
        assert_eq!(merged_status.spec, Some(6));        // 1 + 2 + 3
        assert_eq!(merged_status.incomplete, Some(6));  // 1 + 2 + 3
    }

    #[test]
    fn test_merge_statuses_different_keys_separate_entries() {
        // Test that annotations with different keys get separate status entries
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(1), Some(2), None, None, None));
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(3), Some(4), None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1],
            statuses: statuses1,
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        // Should have two separate status entries
        assert_eq!(status_collection.status_by_key.len(), 2);
        
        let status1 = status_collection.status_by_key.get(&key1).unwrap();
        assert_eq!(status1.spec, Some(1));
        assert_eq!(status1.incomplete, Some(2));
        
        let status2 = status_collection.status_by_key.get(&key2).unwrap();
        assert_eq!(status2.spec, Some(3));
        assert_eq!(status2.incomplete, Some(4));
    }

    #[test]
    fn test_merge_statuses_multiple_annotations_per_report() {
        // Test merging when each report has multiple annotations
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        let key3 = AnnotationKey::from(&anno3);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        statuses1.insert("1".to_string(), create_test_status(Some(2), None, None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(3), None, None, None, None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno3],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        // Should have three separate status entries
        assert_eq!(status_collection.status_by_key.len(), 3);
        
        assert!(status_collection.status_by_key.contains_key(&key1));
        assert!(status_collection.status_by_key.contains_key(&key2));
        assert!(status_collection.status_by_key.contains_key(&key3));
    }

    #[test]
    fn test_merge_statuses_mixed_same_and_different_keys() {
        // Test a realistic scenario with both shared and unique annotations
        let shared_anno = create_test_annotation("src/shared.rs", "spec1", Some("s1"), Some(10));
        let unique_anno1 = create_test_annotation("src/unique1.rs", "spec1", Some("s1"), Some(20));
        let unique_anno2 = create_test_annotation("src/unique2.rs", "spec1", Some("s1"), Some(30));
        
        let shared_key = AnnotationKey::from(&shared_anno);
        let unique_key1 = AnnotationKey::from(&unique_anno1);
        let unique_key2 = AnnotationKey::from(&unique_anno2);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(1), Some(1), None, None, None)); // shared
        statuses1.insert("1".to_string(), create_test_status(Some(2), Some(2), None, None, None)); // unique1
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno.clone(), unique_anno1],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(3), Some(3), None, None, None)); // shared
        statuses2.insert("1".to_string(), create_test_status(Some(4), Some(4), None, None, None)); // unique2
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno, unique_anno2],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        // Should have 3 status entries (1 shared + 2 unique)
        assert_eq!(status_collection.status_by_key.len(), 3);
        
        // Shared annotation should have merged counts
        let shared_status = status_collection.status_by_key.get(&shared_key).unwrap();
        assert_eq!(shared_status.spec, Some(4));        // 1 + 3
        assert_eq!(shared_status.incomplete, Some(4));  // 1 + 3
        
        // Unique annotations should have their original counts
        let unique_status1 = status_collection.status_by_key.get(&unique_key1).unwrap();
        assert_eq!(unique_status1.spec, Some(2));
        assert_eq!(unique_status1.incomplete, Some(2));
        
        let unique_status2 = status_collection.status_by_key.get(&unique_key2).unwrap();
        assert_eq!(unique_status2.spec, Some(4));
        assert_eq!(unique_status2.incomplete, Some(4));
    }

    #[test]
    fn test_merge_statuses_no_statuses_in_report() {
        // Test that reports with annotations but no statuses are handled
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: HashMap::new(), // Empty statuses
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Should have no status entries
        assert_eq!(status_collection.status_by_key.len(), 0);
    }

    #[test]
    fn test_merge_statuses_partial_statuses() {
        // Test when only some annotations have statuses
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let key1 = AnnotationKey::from(&anno1);
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        // No status for annotation 1 (anno2)
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Should have only one status entry
        assert_eq!(status_collection.status_by_key.len(), 1);
        assert!(status_collection.status_by_key.contains_key(&key1));
    }

    #[test]
    fn test_merge_statuses_invalid_id_string() {
        // Test that invalid ID strings are skipped gracefully
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let mut statuses = HashMap::new();
        statuses.insert("not-a-number".to_string(), create_test_status(Some(1), None, None, None, None));
        statuses.insert("0".to_string(), create_test_status(Some(2), None, None, None, None));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Should only have the valid status entry
        assert_eq!(status_collection.status_by_key.len(), 1);
    }

    #[test]
    fn test_merge_statuses_missing_annotation_for_status() {
        // Test when a status references an annotation ID that doesn't exist
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        statuses.insert("99".to_string(), create_test_status(Some(2), None, None, None, None)); // No annotation with ID 99
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Should only have the valid status entry (ID 99 is skipped)
        assert_eq!(status_collection.status_by_key.len(), 1);
    }

    #[test]
    fn test_merge_statuses_all_count_fields() {
        // Test that all count fields are properly merged
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), crate::merge::schema::JsonStatus {
            spec: Some(1),
            incomplete: Some(2),
            citation: Some(3),
            implication: Some(4),
            test: Some(5),
            exception: Some(6),
            todo: Some(7),
            related: None,
        });
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), crate::merge::schema::JsonStatus {
            spec: Some(10),
            incomplete: Some(20),
            citation: Some(30),
            implication: Some(40),
            test: Some(50),
            exception: Some(60),
            todo: Some(70),
            related: None,
        });
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // All fields should be summed
        assert_eq!(merged_status.spec, Some(11));        // 1 + 10
        assert_eq!(merged_status.incomplete, Some(22));  // 2 + 20
        assert_eq!(merged_status.citation, Some(33));    // 3 + 30
        assert_eq!(merged_status.implication, Some(44)); // 4 + 40
        assert_eq!(merged_status.test, Some(55));        // 5 + 50
        assert_eq!(merged_status.exception, Some(66));   // 6 + 60
        assert_eq!(merged_status.todo, Some(77));        // 7 + 70
    }

    #[test]
    fn test_merge_statuses_different_keys_no_interference() {
        // Test that merging different keys doesn't interfere with each other
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        let key3 = AnnotationKey::from(&anno3);
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        statuses.insert("1".to_string(), create_test_status(Some(2), None, None, None, None));
        statuses.insert("2".to_string(), create_test_status(Some(3), None, None, None, None));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Should have three separate entries
        assert_eq!(status_collection.status_by_key.len(), 3);
        
        // Each should have its original value
        assert_eq!(status_collection.status_by_key.get(&key1).unwrap().spec, Some(1));
        assert_eq!(status_collection.status_by_key.get(&key2).unwrap().spec, Some(2));
        assert_eq!(status_collection.status_by_key.get(&key3).unwrap().spec, Some(3));
    }

    #[test]
    fn test_merge_statuses_different_keys_with_related_ids() {
        // Test that different keys maintain separate related ID lists
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let key1 = AnnotationKey::from(&anno1);
        let key2 = AnnotationKey::from(&anno2);
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2]),
        ));
        statuses.insert("1".to_string(), create_test_status(
            Some(2),
            None,
            None,
            None,
            Some(vec![3, 4]),
        ));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let status_collection = merge_statuses(&[report], &collection);
        
        // Each key should have its own related IDs
        let status1 = status_collection.status_by_key.get(&key1).unwrap();
        assert_eq!(status1.related.as_ref().unwrap(), &vec![1, 2]);
        
        let status2 = status_collection.status_by_key.get(&key2).unwrap();
        assert_eq!(status2.related.as_ref().unwrap(), &vec![3, 4]);
    }

    #[test]
    fn test_merge_statuses_empty_statuses_map() {
        // Test that empty statuses map is handled correctly
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1],
            statuses: HashMap::new(), // Empty
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        // Should only have status from report2
        assert_eq!(status_collection.status_by_key.len(), 1);
    }

    #[test]
    fn test_merge_statuses_preserves_zero_counts() {
        // Test that explicit zero counts are preserved
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        let key = AnnotationKey::from(&anno);
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(0), Some(0), None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(1), Some(2), None, None, None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let status_collection = merge_statuses(&[report1, report2], &collection);
        
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        
        // 0 + 1 = 1, 0 + 2 = 2
        assert_eq!(merged_status.spec, Some(1));
        assert_eq!(merged_status.incomplete, Some(2));
    }

}
