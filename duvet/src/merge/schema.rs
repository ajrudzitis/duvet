// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! JSON schema structures for deserializing duvet JSON reports.
//!
//! These structures mirror the JSON format produced by `duvet report --json`
//! and are used for reading and merging multiple reports.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level JSON report structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonReport {
    /// Optional link to blob/source code viewer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_link: Option<String>,
    
    /// Optional link to issue tracker
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_link: Option<String>,
    
    /// Map of specification path to specification details
    pub specifications: HashMap<String, JsonSpecification>,
    
    /// Array of all annotations in the report
    pub annotations: Vec<JsonAnnotation>,
    
    /// Map of annotation ID (as string) to status information
    pub statuses: HashMap<String, JsonStatus>,
    
    /// Lookup table for all possible annotation status combinations
    pub refs: Vec<JsonRefStatus>,
}

/// Specification details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonSpecification {
    /// Optional title of the specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    
    /// Format of the specification (e.g., "ietf", "markdown")
    pub format: String,
    
    /// Array of annotation IDs that are requirements in this specification
    pub requirements: Vec<usize>,
    
    /// Sections within the specification
    pub sections: Vec<JsonSection>,
}

/// Section within a specification
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonSection {
    /// Section identifier
    pub id: String,
    
    /// Section title
    pub title: String,
    
    /// Lines of content in the section
    /// Can be strings or complex nested arrays for annotated lines
    pub lines: Vec<serde_json::Value>,
    
    /// Optional array of requirement annotation IDs in this section
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Vec<usize>>,
}

/// Annotation linking source code to specification requirements
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JsonAnnotation {
    /// Source file path containing the annotation
    pub source: String,
    
    /// Target specification path
    pub target_path: String,
    
    /// Optional target section within the specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_section: Option<String>,
    
    /// Optional line number in the source file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    
    /// Optional annotation type (e.g., "SPEC", "TEST", "EXCEPTION")
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub anno_type: Option<String>,
    
    /// Optional requirement level (e.g., "MUST", "SHOULD", "MAY")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    
    /// Optional comment text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    
    /// Optional feature name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    
    /// Optional tracking issue reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_issue: Option<String>,
    
    /// Optional tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Status information for an annotation
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct JsonStatus {
    /// Count of spec annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<usize>,
    
    /// Count of incomplete annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete: Option<usize>,
    
    /// Count of citation annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<usize>,
    
    /// Count of implication annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implication: Option<usize>,
    
    /// Count of test annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<usize>,
    
    /// Count of exception annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<usize>,
    
    /// Count of todo annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo: Option<usize>,
    
    /// Array of related annotation IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<Vec<usize>>,
}

/// Reference status entry in the refs lookup table
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct JsonRefStatus {
    /// Whether this ref includes a spec annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<bool>,
    
    /// Whether this ref includes a citation annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<bool>,
    
    /// Whether this ref includes an implication annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implication: Option<bool>,
    
    /// Whether this ref includes a test annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<bool>,
    
    /// Whether this ref includes an exception annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<bool>,
    
    /// Whether this ref includes a todo annotation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo: Option<bool>,
    
    /// Optional requirement level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "specifications": {
            "https://example.com/spec": {
                "format": "markdown",
                "requirements": [0, 1],
                "sections": [
                    {
                        "id": "section-1",
                        "title": "Introduction",
                        "lines": ["Line 1", "Line 2"]
                    }
                ]
            }
        },
        "annotations": [
            {
                "source": "src/lib.rs",
                "target_path": "https://example.com/spec",
                "target_section": "section-1",
                "line": 10,
                "type": "SPEC",
                "level": "MUST",
                "comment": "Test requirement"
            }
        ],
        "statuses": {
            "0": {
                "spec": 1,
                "incomplete": 1
            }
        },
        "refs": [
            {},
            {"todo": true},
            {"spec": true}
        ]
    }"#;

    #[test]
    fn test_deserialize_json_report() {
        let report: JsonReport = serde_json::from_str(SAMPLE_JSON)
            .expect("Failed to deserialize sample JSON");
        
        assert_eq!(report.specifications.len(), 1);
        assert_eq!(report.annotations.len(), 1);
        assert_eq!(report.statuses.len(), 1);
        assert_eq!(report.refs.len(), 3);
    }

    #[test]
    fn test_deserialize_json_specification() {
        let json = r#"{
            "format": "ietf",
            "requirements": [0, 1, 2],
            "sections": []
        }"#;
        
        let spec: JsonSpecification = serde_json::from_str(json)
            .expect("Failed to deserialize specification");
        
        assert_eq!(spec.format, "ietf");
        assert_eq!(spec.requirements.len(), 3);
        assert!(spec.title.is_none());
    }

    #[test]
    fn test_deserialize_json_section() {
        let json = r#"{
            "id": "section-1",
            "title": "Test Section",
            "lines": ["Line 1", "Line 2"],
            "requirements": [0, 1]
        }"#;
        
        let section: JsonSection = serde_json::from_str(json)
            .expect("Failed to deserialize section");
        
        assert_eq!(section.id, "section-1");
        assert_eq!(section.title, "Test Section");
        assert_eq!(section.lines.len(), 2);
        assert_eq!(section.requirements.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_deserialize_json_annotation() {
        let json = r#"{
            "source": "src/main.rs",
            "target_path": "https://example.com/spec",
            "target_section": "section-1",
            "line": 42,
            "type": "TEST",
            "level": "SHOULD",
            "comment": "Test comment",
            "feature": "feature-x",
            "tracking_issue": "issue-123",
            "tags": ["tag1", "tag2"]
        }"#;
        
        let anno: JsonAnnotation = serde_json::from_str(json)
            .expect("Failed to deserialize annotation");
        
        assert_eq!(anno.source, "src/main.rs");
        assert_eq!(anno.target_path, "https://example.com/spec");
        assert_eq!(anno.target_section, Some("section-1".to_string()));
        assert_eq!(anno.line, Some(42));
        assert_eq!(anno.anno_type, Some("TEST".to_string()));
        assert_eq!(anno.level, Some("SHOULD".to_string()));
        assert_eq!(anno.comment, Some("Test comment".to_string()));
        assert_eq!(anno.feature, Some("feature-x".to_string()));
        assert_eq!(anno.tracking_issue, Some("issue-123".to_string()));
        assert_eq!(anno.tags.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_deserialize_minimal_annotation() {
        let json = r#"{
            "source": "src/lib.rs",
            "target_path": "https://example.com/spec"
        }"#;
        
        let anno: JsonAnnotation = serde_json::from_str(json)
            .expect("Failed to deserialize minimal annotation");
        
        assert_eq!(anno.source, "src/lib.rs");
        assert_eq!(anno.target_path, "https://example.com/spec");
        assert!(anno.target_section.is_none());
        assert!(anno.line.is_none());
        assert!(anno.anno_type.is_none());
    }

    #[test]
    fn test_deserialize_json_status() {
        let json = r#"{
            "spec": 5,
            "incomplete": 3,
            "citation": 2,
            "test": 1,
            "related": [1, 2, 3]
        }"#;
        
        let status: JsonStatus = serde_json::from_str(json)
            .expect("Failed to deserialize status");
        
        assert_eq!(status.spec, Some(5));
        assert_eq!(status.incomplete, Some(3));
        assert_eq!(status.citation, Some(2));
        assert_eq!(status.test, Some(1));
        assert_eq!(status.related.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_deserialize_empty_status() {
        let json = r#"{}"#;
        
        let status: JsonStatus = serde_json::from_str(json)
            .expect("Failed to deserialize empty status");
        
        assert!(status.spec.is_none());
        assert!(status.incomplete.is_none());
        assert!(status.related.is_none());
    }

    #[test]
    fn test_deserialize_json_ref_status() {
        let json = r#"{
            "spec": true,
            "citation": true,
            "level": "MUST"
        }"#;
        
        let ref_status: JsonRefStatus = serde_json::from_str(json)
            .expect("Failed to deserialize ref status");
        
        assert_eq!(ref_status.spec, Some(true));
        assert_eq!(ref_status.citation, Some(true));
        assert_eq!(ref_status.level, Some("MUST".to_string()));
    }

    #[test]
    fn test_deserialize_empty_ref_status() {
        let json = r#"{}"#;
        
        let ref_status: JsonRefStatus = serde_json::from_str(json)
            .expect("Failed to deserialize empty ref status");
        
        assert!(ref_status.spec.is_none());
        assert!(ref_status.level.is_none());
    }

    #[test]
    fn test_roundtrip_serialization() {
        let original = JsonReport {
            blob_link: Some("https://github.com/example/repo".to_string()),
            issue_link: None,
            specifications: HashMap::new(),
            annotations: vec![
                JsonAnnotation {
                    source: "src/lib.rs".to_string(),
                    target_path: "https://example.com/spec".to_string(),
                    target_section: Some("section-1".to_string()),
                    line: Some(10),
                    anno_type: Some("SPEC".to_string()),
                    level: Some("MUST".to_string()),
                    comment: None,
                    feature: None,
                    tracking_issue: None,
                    tags: None,
                }
            ],
            statuses: HashMap::new(),
            refs: vec![],
        };
        
        let json = serde_json::to_string(&original)
            .expect("Failed to serialize");
        let deserialized: JsonReport = serde_json::from_str(&json)
            .expect("Failed to deserialize");
        
        assert_eq!(deserialized.blob_link, original.blob_link);
        assert_eq!(deserialized.annotations.len(), original.annotations.len());
        assert_eq!(deserialized.annotations[0], original.annotations[0]);
    }

    #[test]
    fn test_deserialize_complex_lines() {
        // Test that we can handle complex nested line structures
        let json = r#"{
            "id": "section-1",
            "title": "Test",
            "lines": [
                "Simple string line",
                [
                    [[], 0, "Text "],
                    [[0], 240, "annotated text"]
                ]
            ]
        }"#;
        
        let section: JsonSection = serde_json::from_str(json)
            .expect("Failed to deserialize section with complex lines");
        
        assert_eq!(section.lines.len(), 2);
        assert!(section.lines[0].is_string());
        assert!(section.lines[1].is_array());
    }

    #[test]
    fn test_deserialize_with_related_ids() {
        let json = r#"{
            "spec": 2,
            "citation": 1,
            "related": [5, 10, 15]
        }"#;
        
        let status: JsonStatus = serde_json::from_str(json)
            .expect("Failed to deserialize status with related IDs");
        
        assert_eq!(status.spec, Some(2));
        assert_eq!(status.citation, Some(1));
        let related = status.related.as_ref().unwrap();
        assert_eq!(related.len(), 3);
        assert_eq!(related[0], 5);
        assert_eq!(related[1], 10);
        assert_eq!(related[2], 15);
    }

    #[test]
    fn test_deserialize_specification_with_title() {
        let json = r#"{
            "title": "RFC 2324",
            "format": "ietf",
            "requirements": [0, 1, 2],
            "sections": []
        }"#;
        
        let spec: JsonSpecification = serde_json::from_str(json)
            .expect("Failed to deserialize specification with title");
        
        assert_eq!(spec.title, Some("RFC 2324".to_string()));
        assert_eq!(spec.format, "ietf");
        assert_eq!(spec.requirements.len(), 3);
    }

    #[test]
    fn test_annotation_equality() {
        let anno1 = JsonAnnotation {
            source: "src/lib.rs".to_string(),
            target_path: "https://example.com/spec".to_string(),
            target_section: Some("section-1".to_string()),
            line: Some(10),
            anno_type: Some("SPEC".to_string()),
            level: Some("MUST".to_string()),
            comment: None,
            feature: None,
            tracking_issue: None,
            tags: None,
        };
        
        let anno2 = anno1.clone();
        
        assert_eq!(anno1, anno2);
    }
}
