// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Merge subcommand for combining multiple JSON reports.

pub mod logic;
pub mod schema;

use crate::Result;
use clap::Parser;
use duvet_core::{diagnostic::{Context, IntoDiagnostic}, path::Path};

#[derive(Debug, Parser)]
pub struct Merge {
    /// JSON report files to merge
    #[clap(required = true)]
    inputs: Vec<Path>,

    #[clap(long)]
    json: Option<Path>,

    #[clap(long)]
    html: Option<Path>,

    #[clap(long)]
    lcov: Option<Path>,
}

impl Merge {
    pub async fn exec(&self) -> Result {
        use duvet_core::progress;

        // Load and parse all input JSON files
        let progress = progress!("Loading JSON reports");
        let reports = self.load_reports().await?;
        progress!(progress, "Loaded {} JSON reports", reports.len());

        // TODO: Implement merge logic
        eprintln!("Merge command not yet fully implemented");
        eprintln!("Successfully loaded {} reports", reports.len());
        eprintln!("JSON output: {:?}", self.json);
        eprintln!("HTML output: {:?}", self.html);
        eprintln!("LCOV output: {:?}", self.lcov);
        Ok(())
    }

    /// Load and parse all input JSON files
    async fn load_reports(&self) -> Result<Vec<schema::JsonReport>> {
        let mut reports = Vec::with_capacity(self.inputs.len());
        let mut errors = Vec::new();

        for input_path in &self.inputs {
            match self.load_single_report(input_path).await {
                Ok(report) => reports.push(report),
                Err(err) => {
                    // Collect errors but continue processing other files
                    eprintln!("Warning: Failed to load {}: {}", input_path, err);
                    errors.push(err);
                }
            }
        }

        // If any files failed, return error with summary
        if !errors.is_empty() {
            return Err(duvet_core::error!(
                "Failed to load {} of {} input files",
                errors.len(),
                self.inputs.len()
            ));
        }

        // Validate we have at least one report
        if reports.is_empty() {
            return Err(duvet_core::error!("No valid JSON reports found"));
        }

        Ok(reports)
    }

    /// Load and parse a single JSON report file
    async fn load_single_report(&self, path: &Path) -> Result<schema::JsonReport> {
        use duvet_core::vfs;

        // Read file contents
        let file = vfs::read_string(path)
            .await
            .wrap_err_with(|| format!("Failed to read input file '{}'", path))?;

        // Parse JSON
        let report: schema::JsonReport = serde_json::from_str(&*file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to parse JSON from '{}'", path))?;

        // Validate schema
        self.validate_report(&report, path)?;

        Ok(report)
    }

    /// Validate that a report has required fields
    fn validate_report(&self, report: &schema::JsonReport, path: &Path) -> Result {
        // Check that specifications map exists (even if empty)
        if report.specifications.is_empty() {
            eprintln!(
                "Warning: Report '{}' has no specifications (this may be intentional)",
                path
            );
        }

        // Check that annotations array exists (even if empty)
        // This is implicitly validated by deserialization

        // Check that statuses map exists (even if empty)
        // This is implicitly validated by deserialization

        // Check that refs array exists (even if empty)
        // This is implicitly validated by deserialization

        // Validate that status keys are valid annotation indices
        for (status_key, _status) in &report.statuses {
            let anno_id: usize = status_key
                .parse()
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "Invalid status key '{}' in '{}': must be a valid annotation index",
                        status_key, path
                    )
                })?;

            if anno_id >= report.annotations.len() {
                return Err(duvet_core::error!(
                    "Invalid status key '{}' in '{}': annotation index {} out of bounds (only {} annotations)",
                    status_key,
                    path,
                    anno_id,
                    report.annotations.len()
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_command_structure() {
        // Test that the Merge struct can be created with required fields
        let merge = Merge {
            inputs: vec![Path::from("test1.json"), Path::from("test2.json")],
            json: Some(Path::from("output.json")),
            html: None,
            lcov: None,
        };

        assert_eq!(merge.inputs.len(), 2);
        assert!(merge.json.is_some());
        assert!(merge.html.is_none());
        assert!(merge.lcov.is_none());
    }

    #[test]
    fn test_merge_cli_parsing() {
        use clap::Parser;
        
        // Test that the merge command can be parsed from CLI arguments
        let args = crate::Arguments::try_parse_from(&[
            "duvet",
            "merge",
            "report1.json",
            "report2.json",
            "--json",
            "output.json",
        ]);
        
        assert!(args.is_ok(), "Failed to parse merge command: {:?}", args.err());
        
        if let Ok(crate::Arguments::Merge(merge)) = args {
            assert_eq!(merge.inputs.len(), 2);
            assert!(merge.json.is_some());
        } else {
            panic!("Expected Merge variant");
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent_file() {
        let merge = Merge {
            inputs: vec![Path::from("nonexistent_file_12345.json")],
            json: None,
            html: None,
            lcov: None,
        };

        let result = merge.load_reports().await;
        assert!(result.is_err(), "Expected error for nonexistent file");
        
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("Failed to load") || err_msg.contains("No valid JSON reports"),
            "Error should mention loading failure: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_load_invalid_json() {
        use std::io::Write;

        // Create a temporary file with invalid JSON
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        let mut file = std::fs::File::create(&temp_path).unwrap();
        write!(file, "{{ invalid json content }}").unwrap();
        drop(file);

        let merge = Merge {
            inputs: vec![Path::from(temp_path.to_str().unwrap())],
            json: None,
            html: None,
            lcov: None,
        };

        let result = merge.load_single_report(&merge.inputs[0]).await;
        assert!(result.is_err(), "Expected error for invalid JSON");
        
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("Failed to parse JSON") || err_msg.contains("parse"),
            "Error should mention JSON parsing failure: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_validate_report_with_invalid_status_key() {
        let report = schema::JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: std::collections::HashMap::new(),
            annotations: vec![
                schema::JsonAnnotation {
                    source: "src/lib.rs".to_string(),
                    target_path: "https://example.com/spec".to_string(),
                    target_section: None,
                    line: Some(10),
                    anno_type: None,
                    level: None,
                    comment: None,
                    feature: None,
                    tracking_issue: None,
                    tags: None,
                }
            ],
            statuses: {
                let mut map = std::collections::HashMap::new();
                // Status key "5" is out of bounds (only 1 annotation at index 0)
                map.insert("5".to_string(), schema::JsonStatus::default());
                map
            },
            refs: vec![],
        };

        let merge = Merge {
            inputs: vec![Path::from("test.json")],
            json: None,
            html: None,
            lcov: None,
        };

        let path = Path::from("test.json");
        let result = merge.validate_report(&report, &path);
        assert!(result.is_err(), "Expected error for out-of-bounds status key");
        
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("out of bounds") || err_msg.contains("Invalid status key"),
            "Error should mention out of bounds status key: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_validate_report_with_non_numeric_status_key() {
        let report = schema::JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: std::collections::HashMap::new(),
            annotations: vec![
                schema::JsonAnnotation {
                    source: "src/lib.rs".to_string(),
                    target_path: "https://example.com/spec".to_string(),
                    target_section: None,
                    line: Some(10),
                    anno_type: None,
                    level: None,
                    comment: None,
                    feature: None,
                    tracking_issue: None,
                    tags: None,
                }
            ],
            statuses: {
                let mut map = std::collections::HashMap::new();
                // Status key "invalid" is not a number
                map.insert("invalid".to_string(), schema::JsonStatus::default());
                map
            },
            refs: vec![],
        };

        let merge = Merge {
            inputs: vec![Path::from("test.json")],
            json: None,
            html: None,
            lcov: None,
        };

        let path = Path::from("test.json");
        let result = merge.validate_report(&report, &path);
        assert!(result.is_err(), "Expected error for non-numeric status key");
        
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("Invalid status key") || err_msg.contains("annotation index"),
            "Error should mention invalid status key: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_validate_valid_report() {
        let report = schema::JsonReport {
            blob_link: Some("https://github.com/example/repo".to_string()),
            issue_link: Some("https://github.com/example/repo/issues".to_string()),
            specifications: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "https://example.com/spec".to_string(),
                    schema::JsonSpecification {
                        title: Some("Test Spec".to_string()),
                        format: "markdown".to_string(),
                        requirements: vec![0],
                        sections: vec![],
                    },
                );
                map
            },
            annotations: vec![
                schema::JsonAnnotation {
                    source: "src/lib.rs".to_string(),
                    target_path: "https://example.com/spec".to_string(),
                    target_section: Some("section-1".to_string()),
                    line: Some(10),
                    anno_type: Some("SPEC".to_string()),
                    level: Some("MUST".to_string()),
                    comment: Some("Test requirement".to_string()),
                    feature: None,
                    tracking_issue: None,
                    tags: None,
                }
            ],
            statuses: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "0".to_string(),
                    schema::JsonStatus {
                        spec: Some(1),
                        incomplete: Some(1),
                        citation: None,
                        implication: None,
                        test: None,
                        exception: None,
                        todo: None,
                        related: None,
                    },
                );
                map
            },
            refs: vec![
                schema::JsonRefStatus::default(),
                schema::JsonRefStatus {
                    spec: Some(true),
                    citation: None,
                    implication: None,
                    test: None,
                    exception: None,
                    todo: None,
                    level: Some("MUST".to_string()),
                },
            ],
        };

        let merge = Merge {
            inputs: vec![Path::from("test.json")],
            json: None,
            html: None,
            lcov: None,
        };

        let path = Path::from("test.json");
        let result = merge.validate_report(&report, &path);
        assert!(result.is_ok(), "Expected valid report to pass validation");
    }

    #[tokio::test]
    async fn test_validate_empty_report() {
        let report = schema::JsonReport {
            blob_link: None,
            issue_link: None,
            specifications: std::collections::HashMap::new(),
            annotations: vec![],
            statuses: std::collections::HashMap::new(),
            refs: vec![],
        };

        let merge = Merge {
            inputs: vec![Path::from("test.json")],
            json: None,
            html: None,
            lcov: None,
        };

        let path = Path::from("test.json");
        let result = merge.validate_report(&report, &path);
        // Empty report should be valid (just a warning is printed)
        assert!(result.is_ok(), "Expected empty report to be valid");
    }

    #[tokio::test]
    async fn test_load_reports_empty_input() {
        let merge = Merge {
            inputs: vec![],
            json: None,
            html: None,
            lcov: None,
        };

        let result = merge.load_reports().await;
        // This should fail at CLI parsing level, but if it gets here, should error
        assert!(result.is_err(), "Expected error for empty input list");
    }
}
