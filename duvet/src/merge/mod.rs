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

        // Perform the merge
        let merged_report = self.merge_reports(&reports)?;

        // Write outputs
        if let Some(json_path) = &self.json {
            self.write_json_output(&merged_report, json_path).await?;
        }

        if let Some(html_path) = &self.html {
            self.write_html_output(&merged_report, html_path).await?;
        }

        if self.lcov.is_some() {
            eprintln!("Warning: LCOV output format is not yet implemented for merge command");
        }

        Ok(())
    }

    /// Perform the merge operation on loaded reports.
    ///
    /// This orchestrates the complete merge workflow:
    /// 1. Collect and deduplicate annotations
    /// 2. Assign new sequential IDs
    /// 3. Merge specifications
    /// 4. Merge statuses
    /// 5. Remap related annotation IDs
    /// 6. Build the final merged report
    fn merge_reports(&self, reports: &[schema::JsonReport]) -> Result<schema::MergedReport> {
        use duvet_core::progress;

        // Phase 1: Collect and deduplicate annotations
        let progress = progress!("Merging annotations");
        let collection = logic::collect_annotations(reports);
        progress!(progress, "Collected {} unique annotations", collection.annotation_map.len());

        // Phase 2: Assign new sequential IDs
        let progress = progress!("Assigning new annotation IDs");
        let assignment = logic::assign_new_ids(&collection);
        progress!(progress, "Assigned IDs to {} annotations", assignment.merged_annotations.len());

        // Phase 3: Merge specifications
        let progress = progress!("Merging specifications");
        let merged_specs = logic::merge_specifications(reports);
        progress!(progress, "Merged {} specifications", merged_specs.len());

        // Phase 4: Merge statuses
        let progress = progress!("Merging statuses");
        let status_collection = logic::merge_statuses(reports, &collection);
        progress!(progress, "Merged {} statuses", status_collection.status_by_key.len());

        // Phase 5: Remap related annotation IDs
        let progress = progress!("Remapping annotation IDs");
        let remapped_statuses = logic::remap_related_ids(&status_collection, &collection, &assignment, reports);
        progress!(progress, "Remapped IDs in {} statuses", remapped_statuses.len());

        // Phase 6: Build the final merged report
        let merged_report = logic::build_merged_report(
            reports,
            &assignment,
            remapped_statuses,
            merged_specs,
        );

        Ok(merged_report)
    }

    /// Write the merged report to a JSON file.
    async fn write_json_output(&self, merged_report: &schema::MergedReport, output_path: &Path) -> Result {
        use duvet_core::progress;
        use std::fs::File;
        use std::io::BufWriter;

        let progress = progress!("Writing JSON output");

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to create output directory for '{}'", output_path))?;
        }

        // Create the output file
        let file = File::create(output_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to create output file '{}'", output_path))?;

        let mut writer = BufWriter::new(file);

        // Serialize to JSON with pretty printing
        serde_json::to_writer_pretty(&mut writer, merged_report)
            .into_diagnostic()
            .wrap_err("Failed to serialize merged report to JSON")?;

        progress!(progress, "Wrote merged report to {}", output_path);

        Ok(())
    }

    /// Write the merged report to an HTML file.
    ///
    /// This generates an HTML file with the merged JSON data embedded in a script tag
    /// and includes the JavaScript viewer for interactive browsing.
    async fn write_html_output(&self, merged_report: &schema::MergedReport, output_path: &Path) -> Result {
        use duvet_core::progress;

        let progress = progress!("Writing HTML output");

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to create output directory for '{}'", output_path))?;
        }

        // Serialize merged report to JSON string
        let json_data = serde_json::to_string(merged_report)
            .into_diagnostic()
            .wrap_err("Failed to serialize merged report to JSON")?;

        // Build HTML with embedded JSON and JavaScript viewer
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Compliance Coverage Report</title>
<script type="application/json" id=result>{}</script>
</head>
<body>
<div id=root></div>
<script>{}</script>
</body>
</html>"#,
            json_data,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/www/public/script.js"))
        );

        // Write HTML to file
        std::fs::write(output_path, html)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write HTML output to '{}'", output_path))?;

        progress!(progress, "Wrote HTML report to {}", output_path);

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

    #[tokio::test]
    async fn test_merge_two_reports_end_to_end() {
        use std::io::Write;

        // Create temporary directory for test files
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create report1.json
        let report1_path = temp_path.join("report1.json");
        let mut file1 = std::fs::File::create(&report1_path).unwrap();
        write!(file1, r#"{{
            "blob_link": "https://github.com/example/repo/blob/main",
            "specifications": {{}},
            "annotations": [
                {{
                    "source": "src/lib.rs",
                    "target_path": "https://example.com/spec",
                    "target_section": "section-1",
                    "line": 10,
                    "type": "SPEC"
                }}
            ],
            "statuses": {{
                "0": {{"spec": 1}}
            }},
            "refs": []
        }}"#).unwrap();
        drop(file1);

        // Create report2.json
        let report2_path = temp_path.join("report2.json");
        let mut file2 = std::fs::File::create(&report2_path).unwrap();
        write!(file2, r#"{{
            "blob_link": "https://github.com/example/repo/blob/main",
            "specifications": {{}},
            "annotations": [
                {{
                    "source": "src/lib.rs",
                    "target_path": "https://example.com/spec",
                    "target_section": "section-1",
                    "line": 10,
                    "type": "SPEC"
                }},
                {{
                    "source": "src/other.rs",
                    "target_path": "https://example.com/spec",
                    "target_section": "section-1",
                    "line": 20,
                    "type": "TEST"
                }}
            ],
            "statuses": {{
                "0": {{"spec": 1}},
                "1": {{"test": 1}}
            }},
            "refs": []
        }}"#).unwrap();
        drop(file2);

        // Create merge command
        let output_path = temp_path.join("merged.json");
        let merge = Merge {
            inputs: vec![
                Path::from(report1_path.to_str().unwrap()),
                Path::from(report2_path.to_str().unwrap()),
            ],
            json: Some(Path::from(output_path.to_str().unwrap())),
            html: None,
            lcov: None,
        };

        // Execute merge
        let result = merge.exec().await;
        assert!(result.is_ok(), "Merge should succeed: {:?}", result.err());

        // Verify output file was created
        assert!(output_path.exists(), "Output file should exist");

        // Read and verify merged output
        let merged_json = std::fs::read_to_string(&output_path).unwrap();
        let merged: schema::JsonReport = serde_json::from_str(&merged_json).unwrap();

        // Verify deduplication: should have 2 annotations (lib.rs deduplicated)
        assert_eq!(merged.annotations.len(), 2);

        // Verify status merging: lib.rs should have spec count of 2
        let status0 = merged.statuses.get("0").unwrap();
        assert_eq!(status0.spec, Some(2));

        // Verify link preservation
        assert_eq!(merged.blob_link, Some("https://github.com/example/repo/blob/main".to_string()));
    }

    #[tokio::test]
    async fn test_merge_three_reports_end_to_end() {
        use std::io::Write;

        // Create temporary directory for test files
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create report1.json
        let report1_path = temp_path.join("report1.json");
        let mut file1 = std::fs::File::create(&report1_path).unwrap();
        write!(file1, r#"{{
            "specifications": {{}},
            "annotations": [
                {{
                    "source": "src/a.rs",
                    "target_path": "spec",
                    "line": 10
                }}
            ],
            "statuses": {{
                "0": {{"spec": 1}}
            }},
            "refs": []
        }}"#).unwrap();
        drop(file1);

        // Create report2.json
        let report2_path = temp_path.join("report2.json");
        let mut file2 = std::fs::File::create(&report2_path).unwrap();
        write!(file2, r#"{{
            "specifications": {{}},
            "annotations": [
                {{
                    "source": "src/b.rs",
                    "target_path": "spec",
                    "line": 20
                }}
            ],
            "statuses": {{
                "0": {{"citation": 1}}
            }},
            "refs": []
        }}"#).unwrap();
        drop(file2);

        // Create report3.json
        let report3_path = temp_path.join("report3.json");
        let mut file3 = std::fs::File::create(&report3_path).unwrap();
        write!(file3, r#"{{
            "specifications": {{}},
            "annotations": [
                {{
                    "source": "src/c.rs",
                    "target_path": "spec",
                    "line": 30
                }}
            ],
            "statuses": {{
                "0": {{"test": 1}}
            }},
            "refs": []
        }}"#).unwrap();
        drop(file3);

        // Create merge command
        let output_path = temp_path.join("merged.json");
        let merge = Merge {
            inputs: vec![
                Path::from(report1_path.to_str().unwrap()),
                Path::from(report2_path.to_str().unwrap()),
                Path::from(report3_path.to_str().unwrap()),
            ],
            json: Some(Path::from(output_path.to_str().unwrap())),
            html: None,
            lcov: None,
        };

        // Execute merge
        let result = merge.exec().await;
        assert!(result.is_ok(), "Merge should succeed: {:?}", result.err());

        // Verify output file was created
        assert!(output_path.exists(), "Output file should exist");

        // Read and verify merged output
        let merged_json = std::fs::read_to_string(&output_path).unwrap();
        let merged: schema::JsonReport = serde_json::from_str(&merged_json).unwrap();

        // Verify all 3 annotations are present
        assert_eq!(merged.annotations.len(), 3);

        // Verify annotations are sorted
        assert_eq!(merged.annotations[0].source, "src/a.rs");
        assert_eq!(merged.annotations[1].source, "src/b.rs");
        assert_eq!(merged.annotations[2].source, "src/c.rs");

        // Verify all statuses are present
        assert_eq!(merged.statuses.len(), 3);
        assert!(merged.statuses.contains_key("0"));
        assert!(merged.statuses.contains_key("1"));
        assert!(merged.statuses.contains_key("2"));
    }
}
