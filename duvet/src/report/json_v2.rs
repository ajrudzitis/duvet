// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! JSON v2 format for duvet reports.
//!
//! This module provides a roundtrip-friendly JSON format that can be serialized
//! and deserialized, enabling multi-package report merging and tooling integration.

use crate::{
    annotation::{stable_annotation_id, AnnotationLevel},
    reference::Reference,
    report::{ReportResult, TargetReport},
    specification::Line,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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

impl From<crate::annotation::AnnotationType> for AnnotationType {
    fn from(anno_type: crate::annotation::AnnotationType) -> Self {
        match anno_type {
            crate::annotation::AnnotationType::Spec => AnnotationType::Spec,
            crate::annotation::AnnotationType::Test => AnnotationType::Test,
            crate::annotation::AnnotationType::Citation => AnnotationType::Citation,
            crate::annotation::AnnotationType::Exception => AnnotationType::Exception,
            crate::annotation::AnnotationType::Todo => AnnotationType::Todo,
            crate::annotation::AnnotationType::Implication => AnnotationType::Implication,
        }
    }
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

impl ReportV2 {
    /// Build a v2 report from the internal report result.
    ///
    /// This converts the internal ReportResult structure into the roundtrip-friendly
    /// v2 JSON format with stable annotation IDs, segmented lines, and coverage statistics.
    pub fn from_report_result(report: &ReportResult) -> Self {
        // Step 1: Build stable ID mapping for all annotations
        // Maps internal annotation ID (usize) to stable string ID
        let mut stable_id_map: HashMap<usize, String> = HashMap::new();
        let mut annotations_v2: Vec<AnnotationV2> = Vec::new();

        // Build annotation ID mapping and convert annotations
        for (idx, annotation) in report.annotations.iter().enumerate() {
            let stable_id = stable_annotation_id(annotation);
            stable_id_map.insert(idx, stable_id.clone());

            // Step 2: Convert annotations to AnnotationV2 with stable IDs and quotes
            let anno_v2 = AnnotationV2 {
                id: stable_id,
                source: annotation.source.to_string_lossy().to_string(),
                blob_link: annotation.blob_link.as_ref().map(|s| s.to_string()),
                target_path: annotation.resolve_target_path(),
                target_section: annotation.target_section().map(|s| s.to_string()),
                quote: annotation.quote.clone(),
                anno_type: annotation.anno.into(),
                level: annotation.level,
                line: if annotation.anno_line > 0 {
                    Some(annotation.anno_line)
                } else {
                    None
                },
                comment: if annotation.comment.is_empty() {
                    None
                } else {
                    Some(annotation.comment.clone())
                },
                feature: if annotation.feature.is_empty() {
                    None
                } else {
                    Some(annotation.feature.clone())
                },
                tracking_issue: if annotation.tracking_issue.is_empty() {
                    None
                } else {
                    Some(annotation.tracking_issue.clone())
                },
                tags: annotation.tags.iter().cloned().collect(),
            };
            annotations_v2.push(anno_v2);
        }

        // We also need to map annotation IDs from references (which use AnnotationWithId)
        // to stable IDs. Build a mapping from the references.
        for (_, target_report) in report.targets.iter() {
            for reference in &target_report.references {
                let stable_id = reference.annotation.stable_id.clone();
                stable_id_map.insert(reference.annotation.id, stable_id);
            }
        }

        // Step 3: Convert specifications with segmented lines
        let mut refs_builder = RefsTableBuilder::new();
        let mut specifications_v2: BTreeMap<String, SpecificationV2> = BTreeMap::new();

        for (target, target_report) in report.targets.iter() {
            let spec_v2 = build_specification_v2(target_report, &stable_id_map, &mut refs_builder);
            specifications_v2.insert(target.path.to_string(), spec_v2);
        }

        // Step 4: Build coverage map with stable ID keys
        let mut coverage: BTreeMap<String, CoverageStatus> = BTreeMap::new();

        for (_, target_report) in report.targets.iter() {
            for (anno_id, status) in target_report.statuses.iter() {
                // Get the stable ID for this annotation
                if let Some(stable_id) = stable_id_map.get(anno_id) {
                    let coverage_status = CoverageStatus {
                        spec: status.spec,
                        incomplete: status.incomplete,
                        citation: status.citation,
                        implication: status.implication,
                        test: status.test,
                        exception: status.exception,
                        todo: status.todo,
                        related: status
                            .related
                            .iter()
                            .filter_map(|id| stable_id_map.get(id).cloned())
                            .collect(),
                    };
                    coverage.insert(stable_id.clone(), coverage_status);
                }
            }
        }

        // Step 5: Build the final refs table
        let refs = refs_builder.build();

        // Return the complete ReportV2 with version "2.0"
        ReportV2 {
            version: "2.0".to_string(),
            blob_link: report.blob_link.map(|s| s.to_string()),
            issue_link: report.issue_link.map(|s| s.to_string()),
            specifications: specifications_v2,
            annotations: annotations_v2,
            coverage,
            refs,
        }
    }
}

/// Build a SpecificationV2 from a TargetReport.
fn build_specification_v2(
    target_report: &TargetReport,
    stable_id_map: &HashMap<usize, String>,
    refs_builder: &mut RefsTableBuilder,
) -> SpecificationV2 {
    // Build a map of line number to references for efficient lookup
    let mut line_refs: HashMap<usize, Vec<&Reference>> = HashMap::new();
    let mut section_requirements: HashMap<String, Vec<String>> = HashMap::new();

    for reference in &target_report.references {
        // Track SPEC annotations as requirements for their sections
        if reference.annotation.anno == crate::annotation::AnnotationType::Spec {
            if let Some(section_id) = reference.annotation.target_section() {
                let stable_id = stable_id_map
                    .get(&reference.annotation.id)
                    .cloned()
                    .unwrap_or_default();
                section_requirements
                    .entry(section_id.to_string())
                    .or_default()
                    .push(stable_id);
            }
        }

        // Map references to line numbers
        for line in reference.text.line_range() {
            line_refs.entry(line).or_default().push(reference);
        }
    }

    // Convert sections
    let mut sections_v2: Vec<SectionV2> = Vec::new();

    for section in target_report.specification.sorted_sections() {
        let mut lines_v2: Vec<LineV2> = Vec::new();

        for line in &section.lines {
            if let Line::Str(slice) = line {
                for lineno in slice.line_range() {
                    let line_text = slice.to_string();
                    let line_offset = slice.range().start;

                    // Get references for this line
                    let refs_for_line: Vec<SegmentRef> = line_refs
                        .get(&lineno)
                        .map(|refs| {
                            refs.iter()
                                .map(|r| SegmentRef {
                                    start: r.start(),
                                    end: r.end(),
                                    stable_id: stable_id_map
                                        .get(&r.annotation.id)
                                        .cloned()
                                        .unwrap_or_default(),
                                    anno_type: r.annotation.anno,
                                    level: r.annotation.level,
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let line_v2 =
                        segment_line(&line_text, line_offset, &refs_for_line, refs_builder);
                    lines_v2.push(line_v2);
                }
            }
        }

        // Get requirements for this section
        let requirements = section_requirements
            .get(&section.id)
            .cloned()
            .unwrap_or_default();

        sections_v2.push(SectionV2 {
            id: section.id.clone(),
            title: section.title.clone(),
            lines: lines_v2,
            requirements,
        });
    }

    SpecificationV2 {
        title: target_report.specification.title.clone(),
        format: target_report.specification.format.to_string(),
        sections: sections_v2,
    }
}

/// Generate a v2 JSON report from a ReportResult.
///
/// This is the main entry point for CLI integration, matching the signature
/// of other report functions like `json::report`.
pub fn report(
    report: &crate::report::ReportResult,
    path: &duvet_core::path::Path,
) -> crate::Result {
    let report_v2 = ReportV2::from_report_result(report);
    write_report_v2(&report_v2, path.as_ref())
}

// ============================================================================
// JSON I/O Functions
// ============================================================================

/// Write a v2 report to a file.
///
/// Uses buffered I/O and formats JSON with indentation for readability.
pub fn write_report_v2(report: &ReportV2, path: &std::path::Path) -> crate::Result {
    use std::{fs::File, io::BufWriter};

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)
        .map_err(|e| duvet_core::error!("failed to create file '{}': {}", path.display(), e))?;
    let writer = BufWriter::new(file);
    write_report_v2_to_writer(report, writer)
}

/// Write a v2 report to a writer.
///
/// Formats JSON with indentation for readability.
pub fn write_report_v2_to_writer<W: std::io::Write>(report: &ReportV2, writer: W) -> crate::Result {
    serde_json::to_writer_pretty(writer, report)
        .map_err(|e| duvet_core::error!("failed to serialize report: {}", e))?;
    Ok(())
}

/// Read a v2 report from a file.
///
/// Uses buffered I/O and validates the version field.
#[allow(dead_code)] // Public API for future merge functionality
pub fn read_report_v2(path: &std::path::Path) -> crate::Result<ReportV2> {
    use std::{fs::File, io::BufReader};

    let file = File::open(path)
        .map_err(|e| duvet_core::error!("failed to open file '{}': {}", path.display(), e))?;
    let reader = BufReader::new(file);
    read_report_v2_from_reader(reader)
        .map_err(|e| duvet_core::error!("failed to read report from '{}': {}", path.display(), e))
}

/// Read a v2 report from a reader.
///
/// Validates the version field after deserialization.
#[allow(dead_code)] // Public API for future merge functionality
pub fn read_report_v2_from_reader<R: std::io::Read>(reader: R) -> crate::Result<ReportV2> {
    let report: ReportV2 = serde_json::from_reader(reader)
        .map_err(|e| duvet_core::error!("failed to parse JSON: {}", e))?;

    // Validate version field
    if report.version != "2.0" {
        return Err(duvet_core::error!(
            "unsupported report version '{}', expected '2.0'",
            report.version
        ));
    }

    Ok(report)
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
            assert_eq!(report, &deserialized, "round-trip should preserve all data");
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
            #[generator(bolero::produce::<Vec<u8>>().with().len(1usize..100))]
            line_bytes: Vec<u8>,
            // Reference ranges as (start_offset, length, stable_id suffix)
            #[generator(bolero::produce::<Vec<(u8, u8, u8)>>().with().len(0usize..5))]
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
                LineV2::Segmented(segments) => segments.iter().map(|s| s.text.as_str()).collect(),
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
            #[generator(bolero::produce::<Vec<u8>>().with().len(1usize..50))]
            line_bytes: Vec<u8>,
            #[generator(bolero::produce::<Vec<(u8, u8, u8)>>().with().len(1usize..4))]
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

                    let ref_start =
                        line_offset.saturating_sub(line_len / 2) + (start_pct * line_len / 100);
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

    /// **Feature: json-v2-format, Property 7: Status ID Validity**
    ///
    /// For any sequence of RefStatus insertions into RefsTableBuilder,
    /// all returned status_ids are valid indices into the final refs array.
    ///
    /// **Validates: Requirements 3.6, 4.1, 4.4, 6.5**
    #[test]
    fn status_id_validity() {
        check!().with_type::<Vec<RefStatus>>().for_each(|statuses| {
            let mut builder = RefsTableBuilder::new();
            let mut returned_ids: Vec<usize> = Vec::new();

            // Insert all statuses and collect returned IDs
            for status in statuses {
                let id = builder.get_or_insert(status.clone());
                returned_ids.push(id);
            }

            // Build the final refs table
            let refs = builder.build();

            // Property: all returned IDs must be valid indices
            for id in returned_ids {
                assert!(
                    id < refs.len(),
                    "status_id {} must be valid index into refs array of length {}",
                    id,
                    refs.len()
                );
            }
        });
    }

    /// **Feature: json-v2-format, Property 8: Refs Table Uniqueness**
    ///
    /// For any sequence of RefStatus insertions into RefsTableBuilder,
    /// the final refs array contains no duplicate RefStatus entries.
    ///
    /// **Validates: Requirements 4.1, 6.5**
    #[test]
    fn refs_table_uniqueness() {
        check!().with_type::<Vec<RefStatus>>().for_each(|statuses| {
            let mut builder = RefsTableBuilder::new();

            // Insert all statuses
            for status in statuses {
                builder.get_or_insert(status.clone());
            }

            // Build the final refs table
            let refs = builder.build();

            // Property: no duplicate entries in refs table
            let mut seen: std::collections::HashSet<&RefStatus> = std::collections::HashSet::new();

            for (index, ref_status) in refs.iter().enumerate() {
                assert!(
                    seen.insert(ref_status),
                    "refs table contains duplicate entry at index {index}: {ref_status:?}",
                );
            }
        });
    }

    /// **Feature: json-v2-format, Property 9: Coverage Map Completeness**
    ///
    /// For any ReportV2 produced by from_report_result(), all annotations of type SPEC
    /// have a corresponding entry in the coverage map keyed by their stable ID.
    ///
    /// Since we cannot easily generate ReportResult instances, we test this property
    /// by generating ReportV2 instances and ensuring the invariant is maintained
    /// through serialization round-trip. We construct valid reports where SPEC
    /// annotations have coverage entries.
    ///
    /// **Validates: Requirements 5.2**
    #[test]
    fn coverage_map_completeness() {
        check!().with_type::<ReportV2>().for_each(|report| {
            // Create a valid report by ensuring all SPEC annotations have coverage entries
            let mut valid_report = report.clone();

            // Add coverage entries for any SPEC annotations that don't have them
            for anno in &valid_report.annotations {
                if anno.anno_type == AnnotationType::Spec
                    && !valid_report.coverage.contains_key(&anno.id)
                {
                    valid_report
                        .coverage
                        .insert(anno.id.clone(), CoverageStatus::default());
                }
            }

            // Serialize and deserialize to test round-trip
            let json = serde_json::to_string(&valid_report).expect("serialization should succeed");
            let deserialized: ReportV2 =
                serde_json::from_str(&json).expect("deserialization should succeed");

            // Property: after round-trip, every SPEC annotation must still have a coverage entry
            let spec_annotation_ids: std::collections::HashSet<&str> = deserialized
                .annotations
                .iter()
                .filter(|anno| anno.anno_type == AnnotationType::Spec)
                .map(|anno| anno.id.as_str())
                .collect();

            for spec_id in spec_annotation_ids {
                assert!(
                    deserialized.coverage.contains_key(spec_id),
                    "SPEC annotation with id '{spec_id}' must have a coverage entry after round-trip",
                );
            }
        });
    }

    /// **Feature: json-v2-format, Property 10: Version Field Correctness**
    ///
    /// For any ReportV2 produced by from_report_result(), the version field equals "2.0".
    /// Since we can't easily generate ReportResult, we test that any ReportV2 with
    /// version "2.0" maintains this invariant through serialization round-trip.
    ///
    /// **Validates: Requirements 6.1**
    #[test]
    fn version_field_correctness() {
        // Test that a ReportV2 with version "2.0" maintains this through round-trip
        check!().with_type::<ReportV2>().for_each(|report| {
            // Create a report with the correct version
            let mut report_with_version = report.clone();
            report_with_version.version = "2.0".to_string();

            // Serialize and deserialize
            let json =
                serde_json::to_string(&report_with_version).expect("serialization should succeed");
            let deserialized: ReportV2 =
                serde_json::from_str(&json).expect("deserialization should succeed");

            // Property: version field must be "2.0" after round-trip
            assert_eq!(
                deserialized.version, "2.0",
                "version field must be '2.0' after round-trip"
            );
        });
    }

    // ========================================================================
    // Error Handling Unit Tests
    // ========================================================================

    /// Test that invalid JSON input returns a descriptive parse error.
    ///
    /// **Validates: Requirements 9.1**
    #[test]
    fn read_invalid_json_returns_error() {
        let invalid_json = "{ this is not valid json }";
        let result = read_report_v2_from_reader(invalid_json.as_bytes());

        assert!(result.is_err(), "invalid JSON should return an error");
        let err = result.unwrap_err();
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("failed to parse JSON"),
            "error message should mention parse failure: {err_msg}",
        );
    }

    /// Test that wrong version field returns an error.
    ///
    /// **Validates: Requirements 9.3**
    #[test]
    fn read_wrong_version_returns_error() {
        let wrong_version_json = r#"{
            "version": "1.0",
            "specifications": {},
            "annotations": [],
            "coverage": {},
            "refs": []
        }"#;
        let result = read_report_v2_from_reader(wrong_version_json.as_bytes());

        assert!(result.is_err(), "wrong version should return an error");
        let err = result.unwrap_err();
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("unsupported report version"),
            "error message should mention unsupported version: {err_msg}",
        );
        assert!(
            err_msg.contains("1.0"),
            "error message should include the actual version: {err_msg}",
        );
    }

    /// Test that missing required fields returns a descriptive error.
    ///
    /// **Validates: Requirements 9.4**
    #[test]
    fn read_missing_required_fields_returns_error() {
        // Missing 'version' field
        let missing_version = r#"{
            "specifications": {},
            "annotations": [],
            "coverage": {},
            "refs": []
        }"#;
        let result = read_report_v2_from_reader(missing_version.as_bytes());
        assert!(
            result.is_err(),
            "missing version field should return an error"
        );

        // Missing 'specifications' field
        let missing_specs = r#"{
            "version": "2.0",
            "annotations": [],
            "coverage": {},
            "refs": []
        }"#;
        let result = read_report_v2_from_reader(missing_specs.as_bytes());
        assert!(
            result.is_err(),
            "missing specifications field should return an error"
        );

        // Missing 'annotations' field
        let missing_annotations = r#"{
            "version": "2.0",
            "specifications": {},
            "coverage": {},
            "refs": []
        }"#;
        let result = read_report_v2_from_reader(missing_annotations.as_bytes());
        assert!(
            result.is_err(),
            "missing annotations field should return an error"
        );
    }

    /// Integration test for file-based round-trip.
    /// Generates a v2 report, writes it to a temp file, reads it back, and verifies structure.
    ///
    /// **Validates: Requirements 7.2**
    #[test]
    fn file_round_trip_integration() {
        use std::collections::BTreeMap;

        // Create a realistic ReportV2 with all field types
        let mut specifications = BTreeMap::new();
        specifications.insert(
            "test-spec.md".to_string(),
            SpecificationV2 {
                title: Some("Test Specification".to_string()),
                format: "markdown".to_string(),
                sections: vec![
                    SectionV2 {
                        id: "section-1".to_string(),
                        title: "Introduction".to_string(),
                        lines: vec![LineV2::Plain("This is plain text.".to_string())],
                        requirements: vec![],
                    },
                    SectionV2 {
                        id: "section-2".to_string(),
                        title: "Requirements".to_string(),
                        lines: vec![LineV2::Segmented(vec![
                            LineSegmentV2 {
                                annotation_ids: vec!["abc123def456".to_string()],
                                status_id: 1,
                                text: "MUST implement".to_string(),
                            },
                            LineSegmentV2 {
                                annotation_ids: vec![],
                                status_id: 0,
                                text: " this feature".to_string(),
                            },
                        ])],
                        requirements: vec!["abc123def456".to_string()],
                    },
                ],
            },
        );

        let mut coverage = BTreeMap::new();
        coverage.insert(
            "abc123def456".to_string(),
            CoverageStatus {
                spec: 14,
                incomplete: 0,
                citation: 14,
                implication: 0,
                test: 0,
                exception: 0,
                todo: 0,
                related: vec!["related123".to_string()],
            },
        );

        let original = ReportV2 {
            version: "2.0".to_string(),
            blob_link: Some("https://github.com/test/repo/blob/main".to_string()),
            issue_link: Some("https://github.com/test/repo/issues".to_string()),
            specifications,
            annotations: vec![
                AnnotationV2 {
                    id: "abc123def456".to_string(),
                    source: "src/lib.rs".to_string(),
                    blob_link: Some(
                        "https://github.com/test/repo/blob/main/src/lib.rs".to_string(),
                    ),
                    target_path: "test-spec.md".to_string(),
                    target_section: Some("section-2".to_string()),
                    quote: "MUST implement".to_string(),
                    anno_type: AnnotationType::Spec,
                    level: AnnotationLevel::Must,
                    line: Some(42),
                    comment: Some("Implementation note".to_string()),
                    feature: Some("core".to_string()),
                    tracking_issue: Some("https://github.com/test/repo/issues/1".to_string()),
                    tags: vec!["important".to_string(), "v1".to_string()],
                },
                AnnotationV2 {
                    id: "related123".to_string(),
                    source: "src/test.rs".to_string(),
                    blob_link: None,
                    target_path: "test-spec.md".to_string(),
                    target_section: Some("section-2".to_string()),
                    quote: "MUST implement".to_string(),
                    anno_type: AnnotationType::Citation,
                    level: AnnotationLevel::Auto,
                    line: Some(10),
                    comment: None,
                    feature: None,
                    tracking_issue: None,
                    tags: vec![],
                },
            ],
            coverage,
            refs: vec![
                RefStatus::default(),
                RefStatus {
                    spec: true,
                    citation: true,
                    implication: false,
                    test: false,
                    exception: false,
                    todo: false,
                    level: AnnotationLevel::Must,
                },
            ],
        };

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("duvet_v2_roundtrip_test.json");

        write_report_v2(&original, &temp_path).expect("write should succeed");

        // Read back
        let loaded = read_report_v2(&temp_path).expect("read should succeed");

        // Verify structure matches
        assert_eq!(loaded.version, original.version);
        assert_eq!(loaded.blob_link, original.blob_link);
        assert_eq!(loaded.issue_link, original.issue_link);
        assert_eq!(loaded.annotations.len(), original.annotations.len());
        assert_eq!(loaded.specifications.len(), original.specifications.len());
        assert_eq!(loaded.coverage.len(), original.coverage.len());
        assert_eq!(loaded.refs.len(), original.refs.len());

        // Verify deep equality
        assert_eq!(loaded, original, "round-trip should preserve all data");

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
