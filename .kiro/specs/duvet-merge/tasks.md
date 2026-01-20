# Tasks: Duvet Merge Subcommand

## Phase 1: Core Infrastructure

- [x] 1. Create merge module structure
  - [x] 1.1 Create `duvet/src/merge.rs` with `Merge` command structure and `exec()` method
  - [x] 1.2 Create `duvet/src/merge/mod.rs` with module organization
  - [x] 1.3 Add `Merge` variant to `Arguments` enum in `duvet/src/lib.rs`
  - [x] 1.4 Wire up merge command execution in `Arguments::exec()`
  - [x] 1.5 Add basic CLI test to verify command is recognized

- [x] 2. Implement JSON schema deserialization
  - [x] 2.1 Create `duvet/src/merge/schema.rs`
  - [x] 2.2 Implement `JsonReport` structure with serde derives
  - [x] 2.3 Implement `JsonSpecification` structure
  - [x] 2.4 Implement `JsonSection` structure
  - [x] 2.5 Implement `JsonAnnotation` structure
  - [x] 2.6 Implement `JsonStatus` structure
  - [x] 2.7 Implement `JsonRefStatus` structure
  - [x] 2.8 Add unit tests for deserialization using sample JSON from integration tests

- [x] 3. Implement file loading and validation
  - [x] 3.1 Implement async file reading for input JSON files using `tokio::fs`
  - [x] 3.2 Implement JSON parsing with `serde_json` and error handling
  - [x] 3.3 Implement basic schema validation (check required fields)
  - [x] 3.4 Add error context for file not found errors
  - [x] 3.5 Add error context for JSON parse errors
  - [x] 3.6 Add error context for schema validation errors
  - [x] 3.7 Add unit test for error handling with invalid inputs

## Phase 2: Annotation Key and ID Remapping

- [ ] 4. Implement AnnotationKey
  - [ ] 4.1 Create `AnnotationKey` struct in `duvet/src/merge/logic.rs`
  - [ ] 4.2 Implement `From<&JsonAnnotation>` for `AnnotationKey`
  - [ ] 4.3 Derive `Hash`, `Eq`, `Ord` traits for use in maps
  - [ ] 4.4 Add unit tests for key generation
  - [ ] 4.5 Add unit tests for key equality and ordering

- [ ] 5. Implement annotation collection and deduplication
  - [ ] 5.1 Implement function to collect annotations from all reports
  - [ ] 5.2 Build `BTreeMap<AnnotationKey, JsonAnnotation>` for deduplication
  - [ ] 5.3 Build `HashMap<(report_index, old_id), AnnotationKey>` for ID tracking
  - [ ] 5.4 Add unit tests for deduplication logic
  - [ ] 5.5 Add unit tests for ID tracking

- [ ] 6. Implement new ID assignment
  - [ ] 6.1 Implement function to assign new sequential IDs to annotations
  - [ ] 6.2 Build `HashMap<AnnotationKey, usize>` for key-to-new-ID mapping
  - [ ] 6.3 Create merged annotations vector with new ordering
  - [ ] 6.4 Add unit tests for ID assignment
  - [ ] 6.5 Add unit tests verifying deterministic ordering

## Phase 3: Status Merging

- [ ] 7. Implement status merging by key
  - [ ] 7.1 Implement function to merge statuses using annotation keys
  - [ ] 7.2 Implement status count addition (spec, incomplete, citation, etc.)
  - [ ] 7.3 Implement related ID collection (before remapping)
  - [ ] 7.4 Add unit tests for status merging with same key
  - [ ] 7.5 Add unit tests for status merging with different keys

- [ ] 8. Implement related ID remapping
  - [ ] 8.1 Implement function to remap related annotation IDs
  - [ ] 8.2 Handle related IDs that span multiple reports
  - [ ] 8.3 Deduplicate remapped related IDs
  - [ ] 8.4 Add unit tests for related ID remapping
  - [ ] 8.5 Add unit tests for cross-report related references

- [ ] 9. Implement final status structure generation
  - [ ] 9.1 Convert status_by_key to status_by_new_id
  - [ ] 9.2 Format status keys as strings for JSON output
  - [ ] 9.3 Add unit tests for final status generation

## Phase 4: Specification and Link Merging

- [ ] 10. Implement specification merging
  - [ ] 10.1 Implement function to merge specifications from all reports
  - [ ] 10.2 Detect duplicate specifications (same path)
  - [ ] 10.3 Validate specification consistency (log warning if different)
  - [ ] 10.4 Add unit tests for specification merging
  - [ ] 10.5 Add unit tests for duplicate detection

- [ ] 11. Implement link conflict resolution
  - [ ] 11.1 Implement `resolve_blob_links()` function
  - [ ] 11.2 Implement `resolve_issue_links()` function
  - [ ] 11.3 Add tests for consistent links (should preserve)
  - [ ] 11.4 Add tests for conflicting links (should omit)
  - [ ] 11.5 Add tests for single link (should preserve)
  - [ ] 11.6 Add tests for all null links (should omit)

- [ ] 12. Implement refs array handling
  - [ ] 12.1 Extract refs array from first input report
  - [ ] 12.2 Optionally validate refs arrays are identical across reports
  - [ ] 12.3 Add unit tests for refs array handling

## Phase 5: JSON Output Generation

- [ ] 13. Implement merged JSON structure
  - [ ] 13.1 Create `MergedReport` structure for serialization
  - [ ] 13.2 Implement conversion from merge logic to `MergedReport`
  - [ ] 13.3 Add serde serialization derives
  - [ ] 13.4 Add unit tests for structure creation

- [ ] 14. Implement JSON output
  - [ ] 14.1 Implement JSON serialization of merged report
  - [ ] 14.2 Write merged JSON to output file
  - [ ] 14.3 Add progress indicator for "Writing JSON output"
  - [ ] 14.4 Add integration test for JSON output with 2 input files
  - [ ] 14.5 Add integration test for JSON output with 3+ input files

## Phase 6: HTML and LCOV Output (Optional)

- [ ] 15. Evaluate HTML/LCOV generation approach
  - [ ] 15.1 Assess complexity of reconstructing `ReportResult` from merged JSON
  - [ ] 15.2 Document decision: reconstruct vs new generators vs JSON-only
  - [ ] 15.3 Create implementation plan based on decision

- [ ] 16. Implement HTML output (if feasible)
  - [ ] 16.1 Implement reconstruction of `ReportResult` from merged JSON OR create new HTML generator
  - [ ] 16.2 Reuse existing `html::report()` function or create new generator
  - [ ] 16.3 Add integration test for HTML output

- [ ] 17. Implement LCOV output (if feasible)
  - [ ] 17.1 Implement LCOV generation from merged JSON
  - [ ] 17.2 Reuse existing `lcov::report()` function or create new generator
  - [ ] 17.3 Add integration test for LCOV output

## Phase 7: Testing and Validation

- [ ] 18. Write unit tests
  - [ ] 18.1 Test AnnotationKey generation and equality
  - [ ] 18.2 Test annotation deduplication
  - [ ] 18.3 Test ID remapping correctness
  - [ ] 18.4 Test status merging for same annotation key
  - [ ] 18.5 Test related ID remapping
  - [ ] 18.6 Test link conflict resolution
  - [ ] 18.7 Test specification merging

- [ ] 19. Write integration tests
  - [ ] 19.1 Test merging 2 simple reports with disjoint annotations
  - [ ] 19.2 Test merging 2 reports with overlapping annotations (same key)
  - [ ] 19.3 Test merging reports with conflicting blob_links
  - [ ] 19.4 Test merging reports with conflicting issue_links
  - [ ] 19.5 Test merging reports with consistent links
  - [ ] 19.6 Test merging reports with related annotations across packages
  - [ ] 19.7 Test error handling for missing files
  - [ ] 19.8 Test error handling for invalid JSON
  - [ ] 19.9 Test error handling for schema mismatches
  - [ ] 19.10 Test merging with empty annotations array
  - [ ] 19.11 Test merging with empty statuses object

- [ ] 20. Write property-based tests
  - [ ] 20.1 Property test: Annotation count preservation (with deduplication)
  - [ ] 20.2 Property test: Status additivity for same annotation key
  - [ ] 20.3 Property test: Merge commutativity (order independence)
  - [ ] 20.4 Property test: Merge associativity (grouping independence)
  - [ ] 20.5 Property test: ID remapping correctness

## Phase 8: Error Handling and Edge Cases

- [ ] 21. Implement robust error handling
  - [ ] 21.1 Handle empty input file list
  - [ ] 21.2 Handle single input file (should work, no merge needed)
  - [ ] 21.3 Handle partial failures (some files fail to load)
  - [ ] 21.4 Add summary reporting (X of Y files merged successfully)
  - [ ] 21.5 Add appropriate exit codes for different failure scenarios

- [ ] 22. Handle edge cases
  - [ ] 22.1 Handle reports with no annotations
  - [ ] 22.2 Handle reports with no statuses
  - [ ] 22.3 Handle reports with no specifications
  - [ ] 22.4 Handle very large reports (memory considerations)
  - [ ] 22.5 Handle duplicate input files (same file listed twice)

## Phase 9: Documentation and Polish

- [ ] 23. Add documentation
  - [ ] 23.1 Add doc comments to public functions in merge module
  - [ ] 23.2 Add module-level documentation with usage examples
  - [ ] 23.3 Add inline comments for complex merge logic
  - [ ] 23.4 Update main README with merge command documentation
  - [ ] 23.5 Add examples to guide documentation if it exists

- [ ] 24. Add progress indicators
  - [ ] 24.1 Add progress for "Loading JSON reports (X/Y)"
  - [ ] 24.2 Add progress for "Merging annotations"
  - [ ] 24.3 Add progress for "Merging statuses"
  - [ ] 24.4 Add progress for "Remapping annotation IDs"
  - [ ] 24.5 Add progress for "Writing output"

- [ ] 25. Final validation
  - [ ] 25.1 Run all unit tests
  - [ ] 25.2 Run all integration tests
  - [ ] 25.3 Run all property-based tests
  - [ ] 25.4 Test with real-world multi-package scenario (if available)
  - [ ] 25.5 Verify merged output loads correctly in existing HTML viewer
  - [ ] 25.6 Performance test with 100 input files
  - [ ] 25.7 Run clippy and fix any warnings
  - [ ] 25.8 Run rustfmt

## Notes

- Tasks marked with `*` are optional (HTML/LCOV support)
- Core functionality (JSON merge) is in Phases 1-5
- Phases are ordered for incremental development
- Each phase can be tested independently
- Property-based tests ensure correctness of merge operations
- The annotation ID remapping is the most complex part and requires careful testing
