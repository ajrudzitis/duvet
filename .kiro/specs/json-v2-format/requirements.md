# Requirements: JSON v2 Format

## Glossary

- **ReportV2**: The top-level data structure representing a v2 JSON report
- **Stable ID**: A 16-character hex string derived from FNV-1a hash of annotation content
- **Line Segmentation**: The process of splitting specification lines at annotation coverage boundaries
- **Refs Table**: A lookup array mapping status indices to reference status combinations
- **Coverage Map**: A mapping from SPEC annotation stable IDs to coverage statistics
- **v1 Format**: The existing streaming JSON format used by the HTML frontend
- **v2 Format**: The new roundtrip-friendly JSON format for tooling and merging

---

## Requirement 1: v2 Data Structures

**User Story:** As a developer, I want well-defined v2 JSON data structures, so that reports can be serialized and deserialized reliably.

### Acceptance Criteria

1. THE ReportV2 struct SHALL contain fields: version, blob_link (optional), issue_link (optional), specifications, annotations, coverage, and refs
2. THE SpecificationV2 struct SHALL contain fields: title (optional), format, and sections
3. THE SectionV2 struct SHALL contain fields: id, title, lines, and requirements
4. THE LineV2 enum SHALL support two variants: Plain (string) and Segmented (array of LineSegmentV2)
5. THE LineSegmentV2 struct SHALL contain fields: annotation_ids, status_id, and text
6. THE AnnotationV2 struct SHALL contain fields: id, source, blob_link (optional), target_path, target_section (optional), quote, type, level, line (optional), comment (optional), feature (optional), tracking_issue (optional), and tags
7. THE CoverageStatus struct SHALL contain fields: spec, incomplete, citation, implication, test, exception, todo, and related
8. THE RefStatus struct SHALL contain fields: spec, citation, implication, test, exception, todo, and level

---

## Requirement 2: Stable Annotation IDs

**User Story:** As a developer merging reports, I want annotations to have stable content-derived IDs, so that the same annotation produces the same ID across independent report runs.

### Acceptance Criteria

1. WHEN generating an annotation ID, THE System SHALL compute FNV-1a 64-bit hash of the composite key (source_path, anno_line, target_path)
2. WHEN formatting the stable ID, THE System SHALL produce a 16-character lowercase hexadecimal string
3. THE stable_annotation_id function SHALL be deterministic: the same annotation always produces the same ID
4. WHEN two annotations have different (source_path, anno_line, target_path) tuples, THE System SHALL produce different stable IDs with high probability

---

## Requirement 3: Line Segmentation

**User Story:** As a report consumer, I want specification lines segmented by annotation coverage, so that I can see exactly which annotations cover each part of the text.

### Acceptance Criteria

1. WHEN a line has no annotation coverage, THE System SHALL represent it as LineV2::Plain containing the line text
2. WHEN a line has annotation coverage, THE System SHALL represent it as LineV2::Segmented containing an array of segments
3. WHEN segmenting a line, THE System SHALL split at all annotation coverage boundaries (start and end positions)
4. WHEN building segments, THE System SHALL ensure the concatenation of all segment texts equals the original line text
5. WHEN building a segment, THE System SHALL include in annotation_ids the stable IDs of all annotations whose coverage range includes that segment
6. WHEN building a segment, THE System SHALL assign a status_id that is a valid index into the refs table

---

## Requirement 4: Refs Table

**User Story:** As a report consumer, I want a compact refs table, so that line segments can reference status combinations efficiently.

### Acceptance Criteria

1. THE refs array SHALL contain RefStatus objects representing unique combinations of coverage flags
2. WHEN a new status combination is encountered, THE System SHALL add it to the refs table and return its index
3. WHEN the same status combination is encountered again, THE System SHALL return the existing index
4. THE status_id in every LineSegmentV2 SHALL be a valid index into the refs array

---

## Requirement 5: Coverage Map

**User Story:** As a report consumer, I want coverage statistics keyed by stable annotation IDs, so that I can track coverage for SPEC annotations across merged reports.

### Acceptance Criteria

1. THE coverage map SHALL use stable annotation IDs as keys
2. WHEN an annotation has type SPEC, THE System SHALL include an entry in the coverage map for that annotation
3. THE CoverageStatus.related field SHALL contain stable IDs of non-SPEC annotations that provide coverage
4. THE coverage statistics (spec, incomplete, citation, etc.) SHALL accurately reflect byte offset counts

---

## Requirement 6: Report Building

**User Story:** As a developer, I want to convert internal ReportResult to ReportV2 format, so that I can generate v2 JSON reports.

### Acceptance Criteria

1. WHEN building a ReportV2, THE System SHALL set the version field to "2.0"
2. WHEN building a ReportV2, THE System SHALL convert all annotations to AnnotationV2 with stable IDs
3. WHEN building a ReportV2, THE System SHALL convert all specifications to SpecificationV2 with segmented lines
4. WHEN building a ReportV2, THE System SHALL build the coverage map with stable ID keys
5. WHEN building a ReportV2, THE System SHALL build the refs table containing all unique status combinations
6. WHEN an annotation has a quote, THE AnnotationV2 SHALL include the quote text for portability

---

## Requirement 7: JSON Serialization

**User Story:** As a developer, I want to serialize ReportV2 to JSON and deserialize JSON back to ReportV2, so that reports can be saved and loaded.

### Acceptance Criteria

1. WHEN serializing a ReportV2, THE System SHALL produce valid JSON conforming to the v2 schema
2. WHEN deserializing JSON, THE System SHALL produce a ReportV2 equivalent to the original
3. WHEN serializing, THE System SHALL use BTreeMap for deterministic key ordering
4. WHEN serializing, THE System SHALL skip optional fields that are None or empty
5. WHEN deserializing JSON with missing optional fields, THE System SHALL use default values

---

## Requirement 8: CLI Integration

**User Story:** As a user, I want a --json-v2 flag on the report command, so that I can generate v2 JSON reports from the command line.

### Acceptance Criteria

1. WHEN the user specifies --json-v2 <path>, THE System SHALL generate a v2 JSON report at the specified path
2. WHEN --json-v2 is specified, THE System SHALL write the report using the v2 format
3. WHEN --json-v2 is not specified, THE System SHALL NOT generate a v2 JSON report
4. THE --json-v2 flag SHALL coexist with existing --json flag (both can be specified)

---

## Requirement 9: Error Handling

**User Story:** As a developer, I want clear error messages when JSON operations fail, so that I can diagnose and fix issues.

### Acceptance Criteria

1. IF JSON deserialization fails, THEN THE System SHALL return an error with the parse error details
2. IF file I/O fails, THEN THE System SHALL return an error with the file path and I/O error details
3. IF the JSON version field is not "2.0", THEN THE System SHALL return an error indicating unsupported version
4. IF required fields are missing during deserialization, THEN THE System SHALL return an error indicating which field is missing

---

## Requirement 10: Backward Compatibility

**User Story:** As a maintainer, I want v2 format to coexist with v1, so that existing HTML frontend functionality is preserved.

### Acceptance Criteria

1. THE existing json.rs (v1 format) SHALL remain unchanged and functional
2. THE v2 format SHALL be implemented in a separate module (json_v2.rs)
3. WHEN generating HTML reports, THE System SHALL continue to use v1 format
4. THE v2 format SHALL be used only when explicitly requested via --json-v2 flag
