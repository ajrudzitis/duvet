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
}
