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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
}
