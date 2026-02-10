// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! JSON v2 format for duvet reports.
//!
//! This module provides a roundtrip-friendly JSON format that can be serialized
//! and deserialized, enabling multi-package report merging and tooling integration.

use crate::annotation::AnnotationLevel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Helper function for skip_serializing_if on zero values
fn is_zero(v: &usize) -> bool {
    *v == 0
}

/// The top-level v2 report structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct ReportV2 {
    /// Format version, always "2.0"
    pub version: String,

    /// Optional link template for source blob URLs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_link: Option<String>,

    /// Optional link template for issue tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_link: Option<String>,

    /// Map of specification path to specification data
    pub specifications: BTreeMap<String, SpecificationV2>,

    /// All annotations in the report
    pub annotations: Vec<AnnotationV2>,

    /// Coverage statistics keyed by stable annotation ID
    pub coverage: BTreeMap<String, CoverageStatus>,

    /// Reference status lookup table
    pub refs: Vec<RefStatus>,
}

/// A specification document in v2 format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct SpecificationV2 {
    /// Optional title of the specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Format of the specification (e.g., "ietf", "markdown")
    pub format: String,

    /// Sections within the specification
    pub sections: Vec<SectionV2>,
}

/// A section within a specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct SectionV2 {
    /// Section identifier
    pub id: String,

    /// Section title
    pub title: String,

    /// Lines in the section, either plain or segmented
    pub lines: Vec<LineV2>,

    /// Requirement identifiers in this section
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<String>,
}

/// A line in a specification section.
///
/// Lines can be either plain text (no annotation coverage) or segmented
/// (split at annotation coverage boundaries).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
#[serde(untagged)]
pub enum LineV2 {
    /// A plain text line with no annotation coverage
    Plain(String),
    /// A line segmented by annotation coverage boundaries
    Segmented(Vec<LineSegmentV2>),
}

/// A segment of a line with annotation coverage information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct LineSegmentV2 {
    /// Stable IDs of annotations covering this segment
    pub annotation_ids: Vec<String>,

    /// Index into the refs table for status information
    pub status_id: usize,

    /// The text content of this segment
    pub text: String,
}

/// An annotation in v2 format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct AnnotationV2 {
    /// Stable content-derived ID (16-char hex)
    pub id: String,

    /// Source file path where the annotation is defined
    pub source: String,

    /// Optional link to the source blob
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_link: Option<String>,

    /// Target specification path
    pub target_path: String,

    /// Optional target section within the specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_section: Option<String>,

    /// Quoted text from the specification
    pub quote: String,

    /// Annotation type
    #[serde(rename = "type")]
    pub anno_type: AnnotationType,

    /// Annotation level (Auto, May, Should, Must)
    pub level: AnnotationLevel,

    /// Line number in the source file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,

    /// Optional comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// Optional feature flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,

    /// Optional tracking issue reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_issue: Option<String>,

    /// Tags associated with the annotation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Annotation type for v2 format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub enum AnnotationType {
    #[serde(rename = "SPEC")]
    Spec,
    #[serde(rename = "TEST")]
    Test,
    #[default]
    #[serde(rename = "CITATION")]
    Citation,
    #[serde(rename = "EXCEPTION")]
    Exception,
    #[serde(rename = "TODO")]
    Todo,
    #[serde(rename = "IMPLICATION")]
    Implication,
}

/// Coverage statistics for a SPEC annotation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct CoverageStatus {
    /// Total bytes in the spec annotation
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spec: usize,

    /// Incomplete (uncovered) bytes
    #[serde(default, skip_serializing_if = "is_zero")]
    pub incomplete: usize,

    /// Bytes covered by citations
    #[serde(default, skip_serializing_if = "is_zero")]
    pub citation: usize,

    /// Bytes covered by implications
    #[serde(default, skip_serializing_if = "is_zero")]
    pub implication: usize,

    /// Bytes covered by tests
    #[serde(default, skip_serializing_if = "is_zero")]
    pub test: usize,

    /// Bytes covered by exceptions
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exception: usize,

    /// Bytes marked as TODO
    #[serde(default, skip_serializing_if = "is_zero")]
    pub todo: usize,

    /// Stable IDs of related (non-SPEC) annotations providing coverage
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
}

/// Reference status flags for line segments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[cfg_attr(test, derive(bolero::TypeGenerator))]
pub struct RefStatus {
    /// Has SPEC annotation coverage
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spec: bool,

    /// Has citation coverage
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub citation: bool,

    /// Has implication coverage
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub implication: bool,

    /// Has test coverage
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,

    /// Has exception coverage
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exception: bool,

    /// Has TODO marker
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub todo: bool,

    /// Maximum annotation level
    #[serde(default, skip_serializing_if = "AnnotationLevel::is_auto")]
    pub level: AnnotationLevel,
}

impl AnnotationLevel {
    /// Returns true if the level is Auto (default)
    pub fn is_auto(&self) -> bool {
        matches!(self, AnnotationLevel::Auto)
    }
}

impl RefStatus {
    /// Apply annotation type and level to this status
    pub fn apply(&mut self, anno_type: crate::annotation::AnnotationType, level: AnnotationLevel) {
        use crate::annotation::AnnotationType;
        self.level = self.level.max(level);
        match anno_type {
            AnnotationType::Spec => self.spec = true,
            AnnotationType::Citation => self.citation = true,
            AnnotationType::Implication => self.implication = true,
            AnnotationType::Test => self.test = true,
            AnnotationType::Exception => self.exception = true,
            AnnotationType::Todo => self.todo = true,
        }
    }
}

/// Builder for the refs table that tracks unique RefStatus combinations.
///
/// Each unique combination of coverage flags gets a unique index.
/// Duplicate combinations return the existing index.
#[derive(Debug, Default)]
pub struct RefsTableBuilder {
    /// Map from RefStatus to its index in the refs array
    status_to_index: std::collections::HashMap<RefStatus, usize>,
    /// The refs array being built
    refs: Vec<RefStatus>,
}

impl RefsTableBuilder {
    /// Create a new builder with an empty status at index 0
    pub fn new() -> Self {
        let mut builder = Self::default();
        // Index 0 is always the empty status (no coverage)
        builder.get_or_insert(RefStatus::default());
        builder
    }

    /// Get the index for a status, inserting it if not present
    pub fn get_or_insert(&mut self, status: RefStatus) -> usize {
        if let Some(&index) = self.status_to_index.get(&status) {
            index
        } else {
            let index = self.refs.len();
            self.status_to_index.insert(status.clone(), index);
            self.refs.push(status);
            index
        }
    }

    /// Build the final refs table
    pub fn build(self) -> Vec<RefStatus> {
        self.refs
    }
}

/// Input reference for line segmentation.
///
/// This struct captures the information needed from a Reference to segment a line.
#[derive(Debug, Clone)]
pub struct SegmentRef {
    /// Start byte offset (absolute, in the specification)
    pub start: usize,
    /// End byte offset (absolute, in the specification)
    pub end: usize,
    /// Stable annotation ID
    pub stable_id: String,
    /// Annotation type
    pub anno_type: crate::annotation::AnnotationType,
    /// Annotation level
    pub level: AnnotationLevel,
}

/// Segment a line based on annotation coverage boundaries.
///
/// Takes a line (as text and its byte offset) and references that overlap with it,
/// and returns either a Plain line (if no references) or a Segmented line with
/// segments split at annotation boundaries.
///
/// # Arguments
/// * `line_text` - The text content of the line
/// * `line_offset` - The byte offset where this line starts in the specification
/// * `refs` - References that overlap with this line
/// * `refs_builder` - Builder for the refs table
///
/// # Returns
/// * `LineV2::Plain` if no references overlap the line
/// * `LineV2::Segmented` with segments split at annotation boundaries
pub fn segment_line(
    line_text: &str,
    line_offset: usize,
    refs: &[SegmentRef],
    refs_builder: &mut RefsTableBuilder,
) -> LineV2 {
    // If no references, return plain line
    if refs.is_empty() {
        return LineV2::Plain(line_text.to_string());
    }

    let line_len = line_text.len();

    // Collect all boundary points within the line
    let mut boundaries = std::collections::BTreeSet::new();
    boundaries.insert(0usize); // Start of line (relative)
    boundaries.insert(line_len); // End of line (relative)

    for r in refs {
        // Clamp reference bounds to line bounds and convert to relative offsets
        let ref_start_rel = r.start.saturating_sub(line_offset).min(line_len);
        let ref_end_rel = r.end.saturating_sub(line_offset).min(line_len);

        if ref_start_rel < line_len {
            boundaries.insert(ref_start_rel);
        }
        if ref_end_rel > 0 && ref_end_rel <= line_len {
            boundaries.insert(ref_end_rel);
        }
    }

    // Convert to sorted vec for iteration
    let boundary_list: Vec<usize> = boundaries.into_iter().collect();

    // Build segments between consecutive boundaries
    let mut segments = Vec::new();

    for window in boundary_list.windows(2) {
        let seg_start = window[0];
        let seg_end = window[1];

        // Skip zero-length segments
        if seg_start >= seg_end {
            continue;
        }

        // Find annotations covering this segment (in absolute coordinates)
        let seg_start_abs = line_offset + seg_start;
        let seg_end_abs = line_offset + seg_end;

        let mut covering_ids = Vec::new();
        let mut ref_status = RefStatus::default();

        for r in refs {
            // A reference covers this segment if the segment is fully within the reference range
            if r.start <= seg_start_abs && seg_end_abs <= r.end {
                covering_ids.push(r.stable_id.clone());
                ref_status.apply(r.anno_type, r.level);
            }
        }

        let status_id = refs_builder.get_or_insert(ref_status);
        let text = line_text[seg_start..seg_end].to_string();

        segments.push(LineSegmentV2 {
            annotation_ids: covering_ids,
            status_id,
            text,
        });
    }

    // If we ended up with a single segment covering the whole line with no annotations,
    // we could return Plain, but the design says to return Segmented if there were refs
    LineV2::Segmented(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bolero::check;

    /// **Feature: json-v2-format, Property 1: Serialization Round Trip**
    ///
    /// For any valid ReportV2 instance, serializing to JSON and deserializing
    /// back produces an equivalent ReportV2.
    ///
    /// **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 7.2**
    #[test]
    fn serialization_round_trip() {
        check!().with_type::<ReportV2>().for_each(|report| {
            let json = serde_json::to_string(report).expect("serialization should succeed");
            let deserialized: ReportV2 =
                serde_json::from_str(&json).expect("deserialization should succeed");
            assert_eq!(
                report, &deserialized,
                "round-trip should preserve all data"
            );
        });
    }

    /// **Feature: json-v2-format, Property 5: Line Segmentation Completeness**
    ///
    /// For any line and set of references, the concatenation of all segment texts
    /// equals the original line text.
    ///
    /// **Validates: Requirements 3.4**
    #[test]
    fn line_segmentation_completeness() {
        // Test input: line text and a list of reference ranges
        // We use ASCII bytes since specification text (RFCs, markdown) is typically ASCII
        #[derive(Debug, Clone, bolero::TypeGenerator)]
        struct TestInput {
            // Line text as ASCII bytes (will be converted to string)
            #[generator(bolero::gen::<Vec<u8>>().with().len(1usize..100))]
            line_bytes: Vec<u8>,
            // Reference ranges as (start_offset, length, stable_id suffix)
            #[generator(bolero::gen::<Vec<(u8, u8, u8)>>().with().len(0usize..5))]
            ref_specs: Vec<(u8, u8, u8)>,
        }

        check!().with_type::<TestInput>().for_each(|input| {
            // Convert bytes to ASCII string (printable chars only)
            let line_text: String = input
                .line_bytes
                .iter()
                .map(|&b| (b % 95 + 32) as char)
                .collect();

            let line_offset = 100usize; // Arbitrary offset for the line
            let line_len = line_text.len();

            // Build refs from the spec, ensuring they overlap with the line
            let refs: Vec<SegmentRef> = input
                .ref_specs
                .iter()
                .enumerate()
                .filter_map(|(i, &(start_delta, len, id_suffix))| {
                    // Create refs that overlap with the line
                    let ref_start = line_offset.saturating_sub(start_delta as usize);
                    let ref_len = (len as usize).max(1);
                    let ref_end = ref_start + ref_len;

                    // Only include refs that actually overlap with the line
                    if ref_end > line_offset && ref_start < line_offset + line_len {
                        Some(SegmentRef {
                            start: ref_start,
                            end: ref_end,
                            stable_id: format!("id_{i}_{id_suffix:02x}"),
                            anno_type: crate::annotation::AnnotationType::Citation,
                            level: AnnotationLevel::Auto,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let mut refs_builder = RefsTableBuilder::new();
            let result = segment_line(&line_text, line_offset, &refs, &mut refs_builder);

            // Property: concatenation of segments equals original line
            let concatenated: String = match &result {
                LineV2::Plain(text) => text.clone(),
                LineV2::Segmented(segments) => {
                    segments.iter().map(|s| s.text.as_str()).collect()
                }
            };

            assert_eq!(
                concatenated, line_text,
                "concatenation of segments must equal original line"
            );
        });
    }

    /// **Feature: json-v2-format, Property 6: Line Segmentation Coverage Accuracy**
    ///
    /// For any line segment, the annotation_ids field contains exactly the stable IDs
    /// of annotations whose coverage range includes that segment's byte range.
    ///
    /// **Validates: Requirements 3.5**
    #[test]
    fn line_segmentation_coverage_accuracy() {
        #[derive(Debug, Clone, bolero::TypeGenerator)]
        struct TestInput {
            // ASCII bytes for predictable byte boundaries
            #[generator(bolero::gen::<Vec<u8>>().with().len(1usize..50))]
            line_bytes: Vec<u8>,
            #[generator(bolero::gen::<Vec<(u8, u8, u8)>>().with().len(1usize..4))]
            ref_specs: Vec<(u8, u8, u8)>,
        }

        check!().with_type::<TestInput>().for_each(|input| {
            // Convert bytes to ASCII string
            let line_text: String = input
                .line_bytes
                .iter()
                .map(|&b| (b % 95 + 32) as char)
                .collect();

            let line_offset = 50usize;
            let line_len = line_text.len();

            // Build refs that definitely overlap with the line
            let refs: Vec<SegmentRef> = input
                .ref_specs
                .iter()
                .enumerate()
                .map(|(i, &(start_pct, end_pct, id_suffix))| {
                    // Use percentages to ensure refs are within/around the line
                    let start_pct = (start_pct as usize) % 150; // 0-149%
                    let end_pct = (end_pct as usize) % 150;
                    let (start_pct, end_pct) = if start_pct <= end_pct {
                        (start_pct, end_pct.max(start_pct + 1))
                    } else {
                        (end_pct, start_pct.max(end_pct + 1))
                    };

                    let ref_start = line_offset.saturating_sub(line_len / 2)
                        + (start_pct * line_len / 100);
                    let ref_end = line_offset.saturating_sub(line_len / 2)
                        + (end_pct * line_len / 100).max(ref_start + 1);

                    SegmentRef {
                        start: ref_start,
                        end: ref_end,
                        stable_id: format!("ref_{i}_{id_suffix:02x}"),
                        anno_type: crate::annotation::AnnotationType::Test,
                        level: AnnotationLevel::Must,
                    }
                })
                .collect();

            let mut refs_builder = RefsTableBuilder::new();
            let result = segment_line(&line_text, line_offset, &refs, &mut refs_builder);

            // For segmented lines, verify each segment's annotation_ids
            if let LineV2::Segmented(segments) = result {
                let mut current_offset = line_offset;

                for segment in &segments {
                    let seg_start = current_offset;
                    let seg_end = current_offset + segment.text.len();

                    // Compute expected annotation IDs for this segment
                    let expected_ids: std::collections::BTreeSet<&str> = refs
                        .iter()
                        .filter(|r| r.start <= seg_start && seg_end <= r.end)
                        .map(|r| r.stable_id.as_str())
                        .collect();

                    let actual_ids: std::collections::BTreeSet<&str> =
                        segment.annotation_ids.iter().map(|s| s.as_str()).collect();

                    assert_eq!(
                        expected_ids, actual_ids,
                        "segment [{seg_start}..{seg_end}] should have correct annotation_ids"
                    );

                    current_offset = seg_end;
                }
            }
        });
    }
}
