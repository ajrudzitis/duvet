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
}


