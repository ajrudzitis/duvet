# Requirements: Duvet Merge Subcommand

## Overview
Add a `duvet merge` subcommand to Duvet that enables merging multiple JSON reports from distributed packages into a single unified report. This addresses the use case where requirements are tracked across many separate packages but need a unified view for compliance tracking.

## User Stories

### 1. Multi-Package Requirements Tracking
As a developer working with distributed packages, I want to merge JSON reports from multiple packages so that I can view all requirement implementations in a single unified report.

### 2. Shell-Based File Discovery
As a user, I want to use shell glob patterns to specify input files so that I can leverage familiar shell expansion (e.g., `duvet merge packages/*/report.json`).

### 3. Comprehensive Annotation Preservation
As a requirements engineer, I want all annotations from all packages to be preserved in the merged report so that I can see implementations and tests even when they exist in different packages.

### 4. Consistent Output Formats
As a user familiar with `duvet report`, I want the merge command to support the same output formats (JSON, HTML, LCOV) so that I have a consistent experience across commands.

## Acceptance Criteria

### 1.1 Command Structure
- The system MUST provide a new `duvet merge` subcommand
- The subcommand MUST accept one or more JSON file paths as positional arguments
- The subcommand MUST support shell glob expansion for file discovery

### 1.2 Input Validation
- The system MUST validate that input files exist
- The system MUST validate that input files contain valid JSON
- The system MUST validate that input files conform to the duvet JSON report schema
- The system MUST provide clear error messages for invalid inputs

### 1.3 Merging Logic
- The system MUST merge all specifications from all input reports
- The system MUST preserve all annotations from all input reports
- The system MUST combine statuses for the same requirement across multiple reports
- The system MUST maintain the relationship between annotations and their source packages
- When all input reports have the same `blob_link` value (or only one report provides it), the system MUST use that value in the merged report
- When input reports have conflicting `blob_link` values, the system MUST omit the `blob_link` field from the merged report
- When all input reports have the same `issue_link` value (or only one report provides it), the system MUST use that value in the merged report
- When input reports have conflicting `issue_link` values, the system MUST omit the `issue_link` field from the merged report

### 1.4 Output Formats
- The system MUST support `--json <path>` flag for JSON output
- The system MUST support `--html <path>` flag for HTML output
- The system MUST support `--lcov <path>` flag for LCOV output
- The system MUST preserve `blob_link` and `issue_link` from input JSON reports (not as command-line flags)

### 1.5 Annotation Deduplication Strategy
- The system MUST keep all annotations even if they reference the same requirement
- The system MUST preserve source information for each annotation to distinguish package origin
- The system MUST aggregate statistics across all packages

### 1.6 Error Handling
- The system MUST handle missing files gracefully with clear error messages
- The system MUST handle malformed JSON with clear error messages
- The system MUST handle schema mismatches with clear error messages
- The system MUST continue processing remaining files if one file fails (with warning)

## Non-Functional Requirements

### 2.1 Compatibility
- The merged output MUST be compatible with existing duvet report viewers
- The JSON schema MUST match the existing duvet JSON report schema

### 2.2 Usability
- The command interface MUST be consistent with existing duvet commands
- Error messages MUST be actionable and clear

### 2.3 Maintainability
- The implementation SHOULD reuse existing report generation code
- The implementation SHOULD follow existing duvet code patterns and structure

## Out of Scope
- Configuration file support for merge operations (future enhancement)
- Filtering or excluding specific packages during merge
- Custom merge strategies or conflict resolution
- Incremental merging or caching of merged results
- Per-package `blob_link` and `issue_link` support in the merged report (future enhancement - current schema only supports single global values)

## Dependencies
- Existing JSON report format and schema
- Existing report generation infrastructure (HTML, LCOV)
- Clap for command-line argument parsing

## Assumptions
- All input JSON files follow the same schema version
- Shell glob expansion is handled by the shell, not the application
- Users have write permissions for output directories
