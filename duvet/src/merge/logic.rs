// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Core merge logic for combining multiple JSON reports.

use super::schema::{JsonAnnotation, JsonReport, JsonSpecification};
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
    
    /// Tracking of related IDs with their source report context
    /// Maps annotation key to list of (report_index, old_related_id) pairs
    pub related_ids_by_key: HashMap<AnnotationKey, Vec<(usize, usize)>>,
}

impl StatusCollection {
    /// Create a new empty status collection
    pub fn new() -> Self {
        Self {
            status_by_key: HashMap::new(),
            related_ids_by_key: HashMap::new(),
        }
    }
}

/// Merge statuses from multiple reports using annotation keys.
///
/// This function:
/// 1. Iterates through all statuses in all reports
/// 2. Maps old annotation IDs to annotation keys using the collection
/// 3. Merges status counts for annotations with the same key
/// 4. Collects related annotation IDs with their report context (before remapping)
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
                .entry(key.clone())
                .and_modify(|existing| {
                    // Add status counts (but not related IDs)
                    add_status_counts_without_related(existing, status);
                })
                .or_insert_with(|| {
                    let mut new_status = status.clone();
                    // Clear related IDs - we'll track them separately
                    new_status.related = None;
                    new_status
                });
            
            // Track related IDs with their report context
            if let Some(related_ids) = &status.related {
                let related_with_context: Vec<(usize, usize)> = related_ids
                    .iter()
                    .map(|&id| (report_index, id))
                    .collect();
                
                status_collection.related_ids_by_key
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .extend(related_with_context);
            }
        }
    }
    
    status_collection
}

/// Add status counts from one status to another (without related IDs).
///
/// This helper function adds all count fields (spec, incomplete, citation, etc.)
/// but does NOT merge related IDs (those are tracked separately with report context).
///
/// # Arguments
/// * `target` - The status to add counts to (modified in place)
/// * `source` - The status to add counts from
fn add_status_counts_without_related(
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
    
    // Note: related IDs are NOT merged here - they're tracked separately
}

/// Add status counts from one status to another (legacy function for tests).
///
/// This helper function adds all count fields (spec, incomplete, citation, etc.)
/// and unions the related IDs arrays.
///
/// # Arguments
/// * `target` - The status to add counts to (modified in place)
/// * `source` - The status to add counts from
#[allow(dead_code)]
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

/// Remap related annotation IDs in statuses from old IDs to new IDs.
///
/// This function:
/// 1. Iterates through all statuses
/// 2. For each related ID (with report context), looks up the annotation key
/// 3. Maps the annotation key to the new ID using the assignment
/// 4. Deduplicates and sorts the remapped IDs
///
/// # Arguments
/// * `status_collection` - The merged statuses with related ID tracking
/// * `collection` - The annotation collection with old-to-key mappings
/// * `assignment` - The ID assignment with key-to-new-ID mappings
/// * `reports` - The original reports (not used in new implementation)
///
/// # Returns
/// A new `HashMap<String, JsonStatus>` with remapped related IDs, keyed by new annotation ID
pub fn remap_related_ids(
    status_collection: &StatusCollection,
    collection: &AnnotationCollection,
    assignment: &IdAssignment,
    _reports: &[JsonReport],
) -> HashMap<String, super::schema::JsonStatus> {
    let mut remapped_statuses = HashMap::new();
    
    for (key, status) in &status_collection.status_by_key {
        // Get the new ID for this annotation
        let new_id = match assignment.key_to_new_id.get(key) {
            Some(id) => *id,
            None => continue, // Skip if key not found (shouldn't happen)
        };
        
        let mut new_status = status.clone();
        
        // Remap related IDs if we have tracking data
        if let Some(related_with_context) = status_collection.related_ids_by_key.get(key) {
            let mut remapped_related = Vec::new();
            
            // For each (report_index, old_related_id) pair
            for &(report_index, old_related_id) in related_with_context {
                // Look up the annotation key for this old ID
                if let Some(related_key) = collection.old_to_key.get(&(report_index, old_related_id)) {
                    // Map the key to the new ID
                    if let Some(&new_related_id) = assignment.key_to_new_id.get(related_key) {
                        remapped_related.push(new_related_id);
                    }
                }
            }
            
            // Deduplicate and sort the remapped IDs
            if !remapped_related.is_empty() {
                remapped_related.sort_unstable();
                remapped_related.dedup();
                new_status.related = Some(remapped_related);
            } else {
                new_status.related = None;
            }
        }
        
        // Store the status with the new ID as the key
        remapped_statuses.insert(new_id.to_string(), new_status);
    }
    
    remapped_statuses
}

/// Merge specifications from multiple reports.
///
/// This function:
/// 1. Iterates through all specifications in all reports
/// 2. Deduplicates specifications by path (keeps first occurrence)
/// 3. Logs warnings if duplicate specifications have different content
/// 4. Returns a merged specifications map
///
/// # Arguments
/// * `reports` - Slice of JSON reports to process
///
/// # Returns
/// A `HashMap<String, JsonSpecification>` containing merged specifications
pub fn merge_specifications(reports: &[JsonReport]) -> HashMap<String, JsonSpecification> {
    let mut merged_specs = HashMap::new();
    
    for (report_index, report) in reports.iter().enumerate() {
        for (spec_path, spec) in &report.specifications {
            if let Some(existing_spec) = merged_specs.get(spec_path) {
                // Specification already exists - check if it's identical
                if !specifications_equal(existing_spec, spec) {
                    eprintln!(
                        "Warning: Specification '{}' differs between reports (using first occurrence from report {})",
                        spec_path,
                        report_index
                    );
                }
            } else {
                // First occurrence of this specification
                merged_specs.insert(spec_path.clone(), spec.clone());
            }
        }
    }
    
    merged_specs
}

/// Check if two specifications are equal (for duplicate detection).
///
/// Compares all fields except the requirements array, which will be
/// updated during the merge process.
///
/// # Arguments
/// * `a` - First specification
/// * `b` - Second specification
///
/// # Returns
/// `true` if specifications are equal, `false` otherwise
fn specifications_equal(a: &JsonSpecification, b: &JsonSpecification) -> bool {
    // Compare title
    if a.title != b.title {
        return false;
    }
    
    // Compare format
    if a.format != b.format {
        return false;
    }
    
    // Compare sections count
    if a.sections.len() != b.sections.len() {
        return false;
    }
    
    // Compare each section
    for (section_a, section_b) in a.sections.iter().zip(b.sections.iter()) {
        if section_a.id != section_b.id {
            return false;
        }
        if section_a.title != section_b.title {
            return false;
        }
        // Note: We don't compare lines content as it's complex nested JSON
        // and requirements arrays will differ anyway
    }
    
    true
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
    fn test_collect_annotations_deduplication() {
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
        
        assert_eq!(collection.annotation_map.len(), 1);
        assert_eq!(collection.old_to_key.len(), 2);
        
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
        
        assert_eq!(collection.annotation_map.len(), 1);
        
        let key = AnnotationKey::from(&anno1);
        let stored_anno = collection.annotation_map.get(&key).unwrap();
        assert_eq!(stored_anno.comment, Some("First comment".to_string()));
        assert_eq!(stored_anno.anno_type, Some("SPEC".to_string()));
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

    // Status merging tests

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
        
        // Related IDs should be tracked separately with report context
        let related_with_context = status_collection.related_ids_by_key.get(&key).unwrap();
        assert_eq!(related_with_context.len(), 5);
        
        // Verify the report context is preserved
        assert!(related_with_context.contains(&(0, 1)));
        assert!(related_with_context.contains(&(0, 2)));
        assert!(related_with_context.contains(&(0, 3)));
        assert!(related_with_context.contains(&(1, 4)));
        assert!(related_with_context.contains(&(1, 5)));
        
        // The status itself should not have related IDs (they're tracked separately)
        let merged_status = status_collection.status_by_key.get(&key).unwrap();
        assert!(merged_status.related.is_none());
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

    // Related ID remapping tests

    #[test]
    fn test_remap_related_ids_single_report_reordered() {
        let anno1 = create_test_annotation("src/z.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/m.rs", "spec1", Some("s1"), Some(30));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2]),
        ));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        let status = remapped.get("2").unwrap();
        let related = status.related.as_ref().unwrap();
        assert_eq!(related, &vec![0, 1]);
    }

    #[test]
    fn test_remap_related_ids_cross_report() {
        // Test remapping when related IDs span multiple reports
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        // Report 1: anno1 and anno2, anno1 relates to anno2 (local ID 1)
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // anno2 in report1 (local ID 1)
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        // Report 2: anno2 and anno3, anno2 relates to anno3 (local ID 1)
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // anno3 in report2 (local ID 1)
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno2, anno3],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2]);
        
        // After sorting:
        // anno1 (src/a.rs) gets new ID 0
        // anno2 (src/b.rs) gets new ID 1 (deduplicated)
        // anno3 (src/c.rs) gets new ID 2
        
        // anno1's status should relate to anno2 (new ID 1)
        let status1 = remapped.get("0").unwrap();
        assert_eq!(status1.related.as_ref().unwrap(), &vec![1]);
        
        // anno2's merged status should relate to anno3 (new ID 2)
        let status2 = remapped.get("1").unwrap();
        assert_eq!(status2.related.as_ref().unwrap(), &vec![2]);
    }

    #[test]
    fn test_remap_related_ids_deduplication() {
        // Test that duplicate related IDs are deduplicated after remapping
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        // Report 1: anno1 relates to anno2
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // anno2 in report1
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1.clone(), anno2.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        // Report 2: same anno1 relates to same anno2 (duplicate)
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // anno2 in report2
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2]);
        
        // After merging, anno1 should have related IDs [1, 1] which should be deduplicated to [1]
        let status = remapped.get("0").unwrap();
        let related = status.related.as_ref().unwrap();
        assert_eq!(related, &vec![1]); // Deduplicated
    }

    #[test]
    fn test_remap_related_ids_multiple_related_cross_report() {
        // Test remapping multiple related IDs that span different reports
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        let anno4 = create_test_annotation("src/d.rs", "spec1", Some("s1"), Some(40));
        
        // Report 1: anno1 and anno2
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 0, 1]), // Relates to anno2 (ID 1) and itself (ID 0), with duplicate
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses: statuses1,
            refs: vec![],
        };
        
        // Report 2: anno3 and anno4
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // Relates to anno4 (ID 1 in report2)
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno3, anno4],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2]);
        
        // After sorting:
        // anno1 (src/a.rs) gets new ID 0
        // anno2 (src/b.rs) gets new ID 1
        // anno3 (src/c.rs) gets new ID 2
        // anno4 (src/d.rs) gets new ID 3
        
        // anno1's status should relate to [0, 1] (deduplicated)
        let status1 = remapped.get("0").unwrap();
        assert_eq!(status1.related.as_ref().unwrap(), &vec![0, 1]);
        
        // anno3's status should relate to [3]
        let status2 = remapped.get("2").unwrap();
        assert_eq!(status2.related.as_ref().unwrap(), &vec![3]);
    }

    #[test]
    fn test_remap_related_ids_invalid_old_id() {
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 99, 100]),
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
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        let status = remapped.get("0").unwrap();
        let related = status.related.as_ref().unwrap();
        assert_eq!(related, &vec![1]);
    }

    #[test]
    fn test_remap_related_ids_maintains_referential_integrity() {
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2]),
        ));
        statuses.insert("1".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![0, 2]),
        ));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Verify all related IDs are valid
        let total_annotations = assignment.merged_annotations.len();
        
        for (_, status) in &remapped {
            if let Some(related) = &status.related {
                for &related_id in related {
                    assert!(related_id < total_annotations, 
                        "Related ID {} is out of bounds (total annotations: {})", 
                        related_id, total_annotations);
                }
            }
        }
    }

    // Cross-report related reference tests

    #[test]
    fn test_remap_related_ids_overlapping_annotations_with_different_relations() {
        // Test when the same annotation appears in multiple reports but with different related IDs
        let shared_anno = create_test_annotation("src/shared.rs", "spec1", Some("s1"), Some(10));
        let dep1 = create_test_annotation("src/dep1.rs", "spec1", Some("s1"), Some(20));
        let dep2 = create_test_annotation("src/dep2.rs", "spec1", Some("s1"), Some(30));
        
        // Report 1: shared_anno relates to dep1
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // dep1
        ));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno.clone(), dep1],
            statuses: statuses1,
            refs: vec![],
        };
        
        // Report 2: shared_anno relates to dep2
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1]), // dep2
        ));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared_anno, dep2],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone()], &collection);
        
        let remapped = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2]);
        
        // After sorting:
        // dep1 (src/dep1.rs) gets new ID 0
        // dep2 (src/dep2.rs) gets new ID 1
        // shared_anno (src/shared.rs) gets new ID 2
        
        // shared_anno should relate to both deps [0, 1]
        let status = remapped.get("2").unwrap();
        let related = status.related.as_ref().unwrap();
        assert_eq!(related, &vec![0, 1]);
    }

    // Final status structure generation tests

    #[test]
    fn test_final_status_generation_keys_are_strings() {
        // Test that the final status structure uses string keys
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), None, None, None, None));
        statuses.insert("1".to_string(), create_test_status(Some(2), None, None, None, None));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Verify keys are strings
        assert!(final_statuses.contains_key("0"));
        assert!(final_statuses.contains_key("1"));
        
        // Verify we can parse them back to usize
        for key in final_statuses.keys() {
            assert!(key.parse::<usize>().is_ok(), "Key '{}' should be parseable as usize", key);
        }
    }

    #[test]
    fn test_final_status_generation_preserves_all_statuses() {
        // Test that all statuses from status_by_key are present in the final output
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), Some(2), Some(3), None, None));
        statuses.insert("1".to_string(), create_test_status(Some(4), Some(5), Some(6), None, None));
        statuses.insert("2".to_string(), create_test_status(Some(7), Some(8), Some(9), None, None));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Should have 3 statuses
        assert_eq!(final_statuses.len(), 3);
        
        // Verify all statuses are present with correct new IDs
        assert!(final_statuses.contains_key("0"));
        assert!(final_statuses.contains_key("1"));
        assert!(final_statuses.contains_key("2"));
        
        // Verify status values are preserved
        let status0 = final_statuses.get("0").unwrap();
        assert_eq!(status0.spec, Some(1));
        assert_eq!(status0.incomplete, Some(2));
        assert_eq!(status0.citation, Some(3));
        
        let status1 = final_statuses.get("1").unwrap();
        assert_eq!(status1.spec, Some(4));
        assert_eq!(status1.incomplete, Some(5));
        assert_eq!(status1.citation, Some(6));
        
        let status2 = final_statuses.get("2").unwrap();
        assert_eq!(status2.spec, Some(7));
        assert_eq!(status2.incomplete, Some(8));
        assert_eq!(status2.citation, Some(9));
    }

    #[test]
    fn test_final_status_generation_with_merged_counts() {
        // Test that merged counts from multiple reports are preserved in final output
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(5), Some(10), Some(15), Some(20), None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(3), Some(7), Some(11), Some(13), None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses2,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2]);
        
        // Should have 1 status with merged counts
        assert_eq!(final_statuses.len(), 1);
        
        let status = final_statuses.get("0").unwrap();
        assert_eq!(status.spec, Some(8));        // 5 + 3
        assert_eq!(status.incomplete, Some(17));  // 10 + 7
        assert_eq!(status.citation, Some(26));    // 15 + 11
        assert_eq!(status.test, Some(33));        // 20 + 13
    }

    #[test]
    fn test_final_status_generation_maps_to_correct_annotation() {
        // Test that status IDs correctly map to their corresponding annotations
        let anno1 = create_test_annotation("src/z.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/m.rs", "spec1", Some("s1"), Some(30));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(100), None, None, None, None)); // anno1 (z.rs)
        statuses.insert("1".to_string(), create_test_status(Some(200), None, None, None, None)); // anno2 (a.rs)
        statuses.insert("2".to_string(), create_test_status(Some(300), None, None, None, None)); // anno3 (m.rs)
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // After sorting by key:
        // anno2 (src/a.rs) gets new ID 0 -> should have spec=200
        // anno3 (src/m.rs) gets new ID 1 -> should have spec=300
        // anno1 (src/z.rs) gets new ID 2 -> should have spec=100
        
        assert_eq!(final_statuses.get("0").unwrap().spec, Some(200)); // a.rs
        assert_eq!(final_statuses.get("1").unwrap().spec, Some(300)); // m.rs
        assert_eq!(final_statuses.get("2").unwrap().spec, Some(100)); // z.rs
        
        // Verify the annotations are in the expected order
        assert_eq!(assignment.merged_annotations[0].source, "src/a.rs");
        assert_eq!(assignment.merged_annotations[1].source, "src/m.rs");
        assert_eq!(assignment.merged_annotations[2].source, "src/z.rs");
    }

    #[test]
    fn test_final_status_generation_empty_statuses() {
        // Test that empty status collections produce empty final output
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: HashMap::new(), // No statuses
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Should be empty
        assert_eq!(final_statuses.len(), 0);
    }

    #[test]
    fn test_final_status_generation_with_remapped_related_ids() {
        // Test that related IDs are correctly remapped in the final output
        let anno1 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(30));
        
        let mut statuses = HashMap::new();
        // anno1 (old ID 0) relates to anno2 (old ID 1) and anno3 (old ID 2)
        statuses.insert("0".to_string(), create_test_status(
            Some(1),
            None,
            None,
            None,
            Some(vec![1, 2]),
        ));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // After sorting:
        // anno2 (src/a.rs) gets new ID 0
        // anno3 (src/b.rs) gets new ID 1
        // anno1 (src/c.rs) gets new ID 2
        
        // anno1's status (now at key "2") should relate to [0, 1] (remapped from [1, 2])
        let status = final_statuses.get("2").unwrap();
        let related = status.related.as_ref().unwrap();
        assert_eq!(related, &vec![0, 1]);
    }

    #[test]
    fn test_final_status_generation_all_annotations_have_status() {
        // Test that every annotation gets a status entry (even if empty)
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        
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
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Should have status for each annotation
        assert_eq!(final_statuses.len(), assignment.merged_annotations.len());
        
        // Verify each annotation ID has a corresponding status
        for i in 0..assignment.merged_annotations.len() {
            let key = i.to_string();
            assert!(final_statuses.contains_key(&key), 
                "Missing status for annotation ID {}", i);
        }
    }

    #[test]
    fn test_final_status_generation_sequential_ids() {
        // Test that final status IDs are sequential starting from 0
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        let anno3 = create_test_annotation("src/c.rs", "spec1", Some("s1"), Some(30));
        let anno4 = create_test_annotation("src/d.rs", "spec1", Some("s1"), Some(40));
        let anno5 = create_test_annotation("src/e.rs", "spec1", Some("s1"), Some(50));
        
        let mut statuses = HashMap::new();
        for i in 0..5 {
            statuses.insert(i.to_string(), create_test_status(Some(i + 1), None, None, None, None));
        }
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2, anno3, anno4, anno5],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Verify IDs are sequential
        let mut ids: Vec<usize> = final_statuses.keys()
            .map(|k| k.parse::<usize>().unwrap())
            .collect();
        ids.sort_unstable();
        
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_final_status_generation_with_deduplication() {
        // Test that when annotations are deduplicated, their statuses are merged correctly
        let anno = create_test_annotation("src/lib.rs", "spec1", Some("s1"), Some(10));
        
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(10), Some(20), None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses1,
            refs: vec![],
        };
        
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(30), Some(40), None, None, None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno.clone()],
            statuses: statuses2,
            refs: vec![],
        };
        
        let mut statuses3 = HashMap::new();
        statuses3.insert("0".to_string(), create_test_status(Some(50), Some(60), None, None, None));
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno],
            statuses: statuses3,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone(), report3.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone(), report3.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2, report3]);
        
        // Should have only 1 status (deduplicated annotation)
        assert_eq!(final_statuses.len(), 1);
        
        // Status should have merged counts from all 3 reports
        let status = final_statuses.get("0").unwrap();
        assert_eq!(status.spec, Some(90));        // 10 + 30 + 50
        assert_eq!(status.incomplete, Some(120)); // 20 + 40 + 60
    }

    #[test]
    fn test_final_status_generation_complex_scenario() {
        // Test a complex scenario with multiple reports, deduplication, and related IDs
        let shared = create_test_annotation("src/shared.rs", "spec1", Some("s1"), Some(10));
        let unique1 = create_test_annotation("src/unique1.rs", "spec1", Some("s1"), Some(20));
        let unique2 = create_test_annotation("src/unique2.rs", "spec1", Some("s1"), Some(30));
        let unique3 = create_test_annotation("src/unique3.rs", "spec1", Some("s1"), Some(40));
        
        // Report 1: shared and unique1, shared relates to unique1
        let mut statuses1 = HashMap::new();
        statuses1.insert("0".to_string(), create_test_status(Some(1), None, None, None, Some(vec![1])));
        statuses1.insert("1".to_string(), create_test_status(Some(2), None, None, None, None));
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared.clone(), unique1],
            statuses: statuses1,
            refs: vec![],
        };
        
        // Report 2: shared and unique2, shared relates to unique2
        let mut statuses2 = HashMap::new();
        statuses2.insert("0".to_string(), create_test_status(Some(3), None, None, None, Some(vec![1])));
        statuses2.insert("1".to_string(), create_test_status(Some(4), None, None, None, None));
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared.clone(), unique2],
            statuses: statuses2,
            refs: vec![],
        };
        
        // Report 3: shared and unique3, shared relates to unique3
        let mut statuses3 = HashMap::new();
        statuses3.insert("0".to_string(), create_test_status(Some(5), None, None, None, Some(vec![1])));
        statuses3.insert("1".to_string(), create_test_status(Some(6), None, None, None, None));
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![shared, unique3],
            statuses: statuses3,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report1.clone(), report2.clone(), report3.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report1.clone(), report2.clone(), report3.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report1, report2, report3]);
        
        // After sorting:
        // shared (src/shared.rs) gets new ID 0
        // unique1 (src/unique1.rs) gets new ID 1
        // unique2 (src/unique2.rs) gets new ID 2
        // unique3 (src/unique3.rs) gets new ID 3
        
        // Should have 4 statuses
        assert_eq!(final_statuses.len(), 4);
        
        // shared's status should have merged counts and relate to all unique annotations
        let shared_status = final_statuses.get("0").unwrap();
        assert_eq!(shared_status.spec, Some(9)); // 1 + 3 + 5
        assert_eq!(shared_status.related.as_ref().unwrap(), &vec![1, 2, 3]);
        
        // Each unique annotation should have its original count
        assert_eq!(final_statuses.get("1").unwrap().spec, Some(2));
        assert_eq!(final_statuses.get("2").unwrap().spec, Some(4));
        assert_eq!(final_statuses.get("3").unwrap().spec, Some(6));
    }

    #[test]
    fn test_final_status_generation_no_related_ids() {
        // Test that statuses without related IDs work correctly
        let anno1 = create_test_annotation("src/a.rs", "spec1", Some("s1"), Some(10));
        let anno2 = create_test_annotation("src/b.rs", "spec1", Some("s1"), Some(20));
        
        let mut statuses = HashMap::new();
        statuses.insert("0".to_string(), create_test_status(Some(1), Some(2), Some(3), Some(4), None));
        statuses.insert("1".to_string(), create_test_status(Some(5), Some(6), Some(7), Some(8), None));
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![anno1, anno2],
            statuses,
            refs: vec![],
        };
        
        let collection = collect_annotations(&[report.clone()]);
        let assignment = assign_new_ids(&collection);
        let status_collection = merge_statuses(&[report.clone()], &collection);
        
        let final_statuses = super::remap_related_ids(&status_collection, &collection, &assignment, &[report]);
        
        // Both statuses should be present
        assert_eq!(final_statuses.len(), 2);
        
        // Verify related fields are None
        assert!(final_statuses.get("0").unwrap().related.is_none());
        assert!(final_statuses.get("1").unwrap().related.is_none());
        
        // Verify counts are preserved
        assert_eq!(final_statuses.get("0").unwrap().spec, Some(1));
        assert_eq!(final_statuses.get("1").unwrap().spec, Some(5));
    }

    // Specification merging tests

    fn create_test_specification(
        title: Option<&str>,
        format: &str,
        requirements: Vec<usize>,
        section_ids: Vec<&str>,
    ) -> crate::merge::schema::JsonSpecification {
        use crate::merge::schema::{JsonSection, JsonSpecification};
        
        let sections = section_ids
            .iter()
            .map(|&id| JsonSection {
                id: id.to_string(),
                title: format!("Section {}", id),
                lines: vec![],
                requirements: None,
            })
            .collect();
        
        JsonSpecification {
            title: title.map(|s| s.to_string()),
            format: format.to_string(),
            requirements,
            sections,
        }
    }

    #[test]
    fn test_merge_specifications_single_report() {
        let mut specs = HashMap::new();
        specs.insert(
            "https://example.com/spec1".to_string(),
            create_test_specification(Some("Spec 1"), "markdown", vec![0, 1], vec!["s1", "s2"]),
        );
        specs.insert(
            "https://example.com/spec2".to_string(),
            create_test_specification(Some("Spec 2"), "ietf", vec![2], vec!["s1"]),
        );
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report]);
        
        assert_eq!(merged.len(), 2);
        assert!(merged.contains_key("https://example.com/spec1"));
        assert!(merged.contains_key("https://example.com/spec2"));
        
        let spec1 = merged.get("https://example.com/spec1").unwrap();
        assert_eq!(spec1.title, Some("Spec 1".to_string()));
        assert_eq!(spec1.format, "markdown");
        assert_eq!(spec1.sections.len(), 2);
    }

    #[test]
    fn test_merge_specifications_multiple_reports_disjoint() {
        let mut specs1 = HashMap::new();
        specs1.insert(
            "https://example.com/spec1".to_string(),
            create_test_specification(Some("Spec 1"), "markdown", vec![0], vec!["s1"]),
        );
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert(
            "https://example.com/spec2".to_string(),
            create_test_specification(Some("Spec 2"), "ietf", vec![0], vec!["s1"]),
        );
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should have both specifications
        assert_eq!(merged.len(), 2);
        assert!(merged.contains_key("https://example.com/spec1"));
        assert!(merged.contains_key("https://example.com/spec2"));
    }

    #[test]
    fn test_merge_specifications_empty_reports() {
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should be empty
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_merge_specifications_preserves_all_fields() {
        let mut specs = HashMap::new();
        specs.insert(
            "https://example.com/spec".to_string(),
            create_test_specification(Some("Test Spec"), "markdown", vec![0, 1, 2], vec!["s1", "s2", "s3"]),
        );
        
        let report = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report]);
        
        let spec = merged.get("https://example.com/spec").unwrap();
        assert_eq!(spec.title, Some("Test Spec".to_string()));
        assert_eq!(spec.format, "markdown");
        assert_eq!(spec.requirements, vec![0, 1, 2]);
        assert_eq!(spec.sections.len(), 3);
        assert_eq!(spec.sections[0].id, "s1");
        assert_eq!(spec.sections[1].id, "s2");
        assert_eq!(spec.sections[2].id, "s3");
    }

    #[test]
    fn test_merge_specifications_duplicate_same_content() {
        // Test that duplicate specifications with identical content are handled correctly
        let spec = create_test_specification(Some("Spec 1"), "markdown", vec![0, 1], vec!["s1", "s2"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/spec".to_string(), spec.clone());
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/spec".to_string(), spec.clone());
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should have only one specification (deduplicated)
        assert_eq!(merged.len(), 1);
        assert!(merged.contains_key("https://example.com/spec"));
        
        // Should preserve the specification content
        let merged_spec = merged.get("https://example.com/spec").unwrap();
        assert_eq!(merged_spec.title, Some("Spec 1".to_string()));
        assert_eq!(merged_spec.format, "markdown");
        assert_eq!(merged_spec.sections.len(), 2);
    }

    #[test]
    fn test_merge_specifications_duplicate_different_content() {
        // Test that duplicate specifications with different content trigger a warning
        // (but still use the first occurrence)
        let spec1 = create_test_specification(Some("Spec 1 Version A"), "markdown", vec![0], vec!["s1"]);
        let spec2 = create_test_specification(Some("Spec 1 Version B"), "markdown", vec![0], vec!["s1"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/spec".to_string(), spec1.clone());
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/spec".to_string(), spec2);
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should have only one specification (first occurrence)
        assert_eq!(merged.len(), 1);
        
        // Should use the first version
        let merged_spec = merged.get("https://example.com/spec").unwrap();
        assert_eq!(merged_spec.title, Some("Spec 1 Version A".to_string()));
    }

    #[test]
    fn test_merge_specifications_duplicate_different_format() {
        // Test detection of specifications with same path but different format
        let spec1 = create_test_specification(Some("Spec 1"), "markdown", vec![0], vec!["s1"]);
        let spec2 = create_test_specification(Some("Spec 1"), "ietf", vec![0], vec!["s1"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/spec".to_string(), spec1.clone());
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/spec".to_string(), spec2);
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should use first occurrence
        let merged_spec = merged.get("https://example.com/spec").unwrap();
        assert_eq!(merged_spec.format, "markdown");
    }

    #[test]
    fn test_merge_specifications_duplicate_different_sections() {
        // Test detection of specifications with different section counts
        let spec1 = create_test_specification(Some("Spec 1"), "markdown", vec![0], vec!["s1", "s2"]);
        let spec2 = create_test_specification(Some("Spec 1"), "markdown", vec![0], vec!["s1", "s2", "s3"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/spec".to_string(), spec1.clone());
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/spec".to_string(), spec2);
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should use first occurrence (2 sections)
        let merged_spec = merged.get("https://example.com/spec").unwrap();
        assert_eq!(merged_spec.sections.len(), 2);
    }

    #[test]
    fn test_merge_specifications_multiple_duplicates_across_reports() {
        // Test merging when the same specification appears in all reports
        let spec = create_test_specification(Some("Shared Spec"), "markdown", vec![0], vec!["s1"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/spec".to_string(), spec.clone());
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/spec".to_string(), spec.clone());
        
        let mut specs3 = HashMap::new();
        specs3.insert("https://example.com/spec".to_string(), spec);
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let report3 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs3,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2, report3]);
        
        // Should have only one specification
        assert_eq!(merged.len(), 1);
        assert!(merged.contains_key("https://example.com/spec"));
    }

    #[test]
    fn test_merge_specifications_mixed_unique_and_duplicate() {
        // Test merging with a mix of unique and duplicate specifications
        let shared_spec = create_test_specification(Some("Shared"), "markdown", vec![0], vec!["s1"]);
        let unique_spec1 = create_test_specification(Some("Unique 1"), "ietf", vec![1], vec!["s1"]);
        let unique_spec2 = create_test_specification(Some("Unique 2"), "markdown", vec![2], vec!["s1"]);
        
        let mut specs1 = HashMap::new();
        specs1.insert("https://example.com/shared".to_string(), shared_spec.clone());
        specs1.insert("https://example.com/unique1".to_string(), unique_spec1);
        
        let report1 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs1,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let mut specs2 = HashMap::new();
        specs2.insert("https://example.com/shared".to_string(), shared_spec);
        specs2.insert("https://example.com/unique2".to_string(), unique_spec2);
        
        let report2 = JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: specs2,
            annotations: vec![],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let merged = super::merge_specifications(&[report1, report2]);
        
        // Should have 3 specifications (1 shared + 2 unique)
        assert_eq!(merged.len(), 3);
        assert!(merged.contains_key("https://example.com/shared"));
        assert!(merged.contains_key("https://example.com/unique1"));
        assert!(merged.contains_key("https://example.com/unique2"));
    }

    #[test]
    fn test_specifications_equal_identical() {
        let spec1 = create_test_specification(Some("Test"), "markdown", vec![0, 1], vec!["s1", "s2"]);
        let spec2 = create_test_specification(Some("Test"), "markdown", vec![0, 1], vec!["s1", "s2"]);
        
        assert!(super::specifications_equal(&spec1, &spec2));
    }

    #[test]
    fn test_specifications_equal_different_title() {
        let spec1 = create_test_specification(Some("Test A"), "markdown", vec![0], vec!["s1"]);
        let spec2 = create_test_specification(Some("Test B"), "markdown", vec![0], vec!["s1"]);
        
        assert!(!super::specifications_equal(&spec1, &spec2));
    }

    #[test]
    fn test_specifications_equal_different_format() {
        let spec1 = create_test_specification(Some("Test"), "markdown", vec![0], vec!["s1"]);
        let spec2 = create_test_specification(Some("Test"), "ietf", vec![0], vec!["s1"]);
        
        assert!(!super::specifications_equal(&spec1, &spec2));
    }

    #[test]
    fn test_specifications_equal_different_section_count() {
        let spec1 = create_test_specification(Some("Test"), "markdown", vec![0], vec!["s1"]);
        let spec2 = create_test_specification(Some("Test"), "markdown", vec![0], vec!["s1", "s2"]);
        
        assert!(!super::specifications_equal(&spec1, &spec2));
    }

    #[test]
    fn test_specifications_equal_different_section_ids() {
        let spec1 = create_test_specification(Some("Test"), "markdown", vec![0], vec!["s1", "s2"]);
        let spec2 = create_test_specification(Some("Test"), "markdown", vec![0], vec!["s1", "s3"]);
        
        assert!(!super::specifications_equal(&spec1, &spec2));
    }

    #[test]
    fn test_specifications_equal_ignores_requirements_array() {
        // Requirements array differences should not affect equality
        // (they will be updated during merge)
        let spec1 = create_test_specification(Some("Test"), "markdown", vec![0, 1], vec!["s1"]);
        let spec2 = create_test_specification(Some("Test"), "markdown", vec![2, 3, 4], vec!["s1"]);
        
        // Should still be equal (requirements are ignored in comparison)
        assert!(super::specifications_equal(&spec1, &spec2));
    }
}
