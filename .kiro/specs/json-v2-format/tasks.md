# Implementation Plan: JSON v2 Format

## Overview

This plan implements a roundtrip-friendly JSON v2 format for duvet reports. The implementation uses Rust with serde derive for bidirectional serialization. Tasks are ordered to build incrementally: data structures first, then conversion logic, then I/O, and finally CLI integration.

## Tasks

- [ ] 1. Define v2 data structures
  - [ ] 1.1 Create `duvet/src/report/json_v2.rs` module with ReportV2, SpecificationV2, SectionV2, LineV2, LineSegmentV2, AnnotationV2, CoverageStatus, and RefStatus structs
    - Add Apache-2.0 license header
    - Use `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]` for all structs
    - Use `#[serde(skip_serializing_if = "...")]` for optional fields
    - Use `BTreeMap` for specifications and coverage maps
    - Use `#[serde(untagged)]` for LineV2 enum
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8_

  - [ ] 1.2 Write property test for serialization round-trip
    - **Property 1: Serialization Round Trip**
    - Generate arbitrary ReportV2 instances using bolero
    - Verify serialize then deserialize equals original
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 7.2**

- [ ] 2. Implement stable annotation ID generation
  - [ ] 2.1 Verify existing `stable_annotation_id()` in `annotation.rs` meets requirements
    - Confirm FNV-1a 64-bit hash of (source_path, anno_line, target_path)
    - Confirm 16-character lowercase hex output
    - _Requirements: 2.1, 2.2_

  - [ ]* 2.2 Write property tests for stable ID
    - **Property 2: Stable ID Format** - verify 16 lowercase hex chars
    - **Property 3: Stable ID Determinism** - same input produces same output
    - **Property 4: Stable ID Uniqueness** - different inputs produce different outputs
    - **Validates: Requirements 2.2, 2.3, 2.4**

- [ ] 3. Checkpoint - Ensure data structures compile and tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement line segmentation
  - [ ] 4.1 Implement `segment_line()` function in `json_v2.rs`
    - Take line slice and references as input
    - Collect boundary points from reference start/end positions
    - Build segments between consecutive boundaries
    - Return `LineV2::Plain` if no references, `LineV2::Segmented` otherwise
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

  - [ ] 4.2 Write property tests for line segmentation
    - **Property 5: Line Segmentation Completeness** - concatenation equals original
    - **Property 6: Line Segmentation Coverage Accuracy** - annotation_ids are correct
    - **Validates: Requirements 3.4, 3.5**

- [ ] 5. Implement refs table builder
  - [ ] 5.1 Implement `RefsTableBuilder` struct in `json_v2.rs`
    - Track unique RefStatus combinations
    - Return existing index for duplicate combinations
    - Build final Vec<RefStatus> on completion
    - _Requirements: 4.1, 4.2, 4.3_

  - [ ] 5.2 Write property tests for refs table
    - **Property 7: Status ID Validity** - all status_ids are valid indices
    - **Property 8: Refs Table Uniqueness** - no duplicate entries
    - **Validates: Requirements 3.6, 4.1, 4.4, 6.5**

- [ ] 6. Checkpoint - Ensure segmentation and refs table work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement report conversion
  - [ ] 7.1 Implement `ReportV2::from_report_result()` method
    - Build stable ID mapping for all annotations
    - Convert annotations to AnnotationV2 with stable IDs and quotes
    - Convert specifications with segmented lines
    - Build coverage map with stable ID keys
    - Set version to "2.0"
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_

  - [ ] 7.2 Write property tests for report conversion
    - **Property 9: Coverage Map Completeness** - all SPEC annotations have coverage entries
    - **Property 10: Version Field Correctness** - version is "2.0"
    - **Validates: Requirements 5.2, 6.1**

- [ ] 8. Implement JSON I/O functions
  - [ ] 8.1 Implement `write_report_v2()` and `read_report_v2()` functions
    - Use serde_json for serialization/deserialization
    - Use buffered I/O for file operations
    - Format JSON with indentation for readability
    - _Requirements: 7.1, 7.3_

  - [ ] 8.2 Implement error handling for I/O operations
    - Return descriptive errors for parse failures
    - Return descriptive errors for I/O failures
    - Validate version field on read
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [ ] 8.3 Write unit tests for error handling
    - Test invalid JSON input
    - Test wrong version field
    - Test missing required fields
    - _Requirements: 9.1, 9.3, 9.4_

- [ ] 9. Checkpoint - Ensure I/O functions work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 10. Integrate with CLI
  - [ ] 10.1 Add `--json-v2` flag to Report struct in `report.rs`
    - Add `json_v2: Option<Path>` field with `#[clap(long)]`
    - _Requirements: 8.1_

  - [ ] 10.2 Wire up v2 report generation in `Report::exec()`
    - Check if `json_v2` path is specified
    - Build ReportV2 from ReportResult
    - Write to specified path
    - Ensure v1 JSON generation still works when `--json` is specified
    - _Requirements: 8.2, 8.3, 8.4, 10.1, 10.3, 10.4_

  - [ ] 10.3 Register json_v2 module in `report/mod.rs`
    - Add `mod json_v2;` declaration
    - _Requirements: 10.2_

- [ ] 11. Add integration tests
  - [ ] 11.1 Create integration test config for v2 JSON output
    - Add test config in `integration/` directory
    - Generate v2 JSON report
    - Add snapshot test for v2 JSON structure
    - _Requirements: 8.1, 8.2_

  - [ ] 11.2 Write integration test for round-trip
    - Generate v2 report, read it back, verify structure
    - _Requirements: 7.2_

- [ ] 12. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests use `bolero` crate (already in project)
- The existing `stable_annotation_id()` function in `annotation.rs` should be reused
- v1 JSON format in `json.rs` remains unchanged for HTML frontend compatibility
