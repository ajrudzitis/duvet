# Design: Duvet Merge Subcommand

## Overview
This document describes the design for implementing a `duvet merge` subcommand that merges multiple JSON reports from distributed packages into a unified report.

## Architecture

### High-Level Flow
```
Input JSON Files → Parse & Validate → Merge Data Structures → Generate Output Reports
```

### Component Structure

#### 1. Merge Module (`duvet/src/merge.rs`)
New top-level module that implements the merge subcommand.

**Key Responsibilities:**
- Command-line argument parsing
- Orchestrating the merge workflow
- Delegating to merge logic and report generation

#### 2. Merge Logic (`duvet/src/merge/logic.rs`)
Core merging functionality.

**Key Responsibilities:**
- Deserializing JSON reports
- Merging specifications, annotations, and statuses
- Handling annotation ID remapping
- Handling link conflicts
- Building unified merged JSON

#### 3. JSON Schema (`duvet/src/merge/schema.rs`)
Deserialization structures for reading JSON reports.

**Key Responsibilities:**
- Serde structures matching the JSON report schema
- Validation logic
- Conversion to internal data structures

## Data Flow

### Input Phase
1. Accept file paths as command-line arguments
2. Read each JSON file from disk in parallel
3. Deserialize into intermediate structures
4. Validate schema compliance

### Merge Phase
1. Collect all unique annotations (deduplicated by content, not ID)
2. Build annotation key → new ID mapping
3. Collect all specifications from all reports
4. Merge statuses using annotation keys, then remap to new IDs
5. Remap `related` annotation IDs in statuses
6. Resolve link conflicts (use if consistent, omit if conflicting)
7. Build unified merged JSON structure

### Output Phase
1. Serialize merged data to JSON
2. For HTML/LCOV: deserialize merged JSON and use existing generators

## Detailed Design

### Command-Line Interface

```rust
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
```

**Usage Examples:**
```bash
# Merge specific files
duvet merge report1.json report2.json --json merged.json --html merged.html

# Using shell glob expansion
duvet merge packages/*/report.json --json merged.json

# Multiple output formats
duvet merge pkg-*/duvet-report.json --json out.json --html out.html --lcov out.lcov
```

### JSON Schema Structures

```rust
#[derive(Debug, Deserialize)]
pub struct JsonReport {
    pub blob_link: Option<String>,
    pub issue_link: Option<String>,
    pub specifications: HashMap<String, JsonSpecification>,
    pub annotations: Vec<JsonAnnotation>,
    pub statuses: HashMap<String, JsonStatus>,
    pub refs: Vec<JsonRefStatus>,
}

#[derive(Debug, Deserialize)]
pub struct JsonSpecification {
    pub title: Option<String>,
    pub format: String,
    pub requirements: Vec<usize>,
    pub sections: Vec<JsonSection>,
}

#[derive(Debug, Deserialize)]
pub struct JsonSection {
    pub id: String,
    pub title: String,
    pub lines: Vec<serde_json::Value>, // Complex nested structure
    pub requirements: Option<Vec<usize>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonAnnotation {
    pub source: String,
    pub target_path: String,
    pub target_section: Option<String>,
    pub line: Option<usize>,
    #[serde(rename = "type")]
    pub anno_type: Option<String>,
    pub level: Option<String>,
    pub comment: Option<String>,
    pub feature: Option<String>,
    pub tracking_issue: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonStatus {
    pub spec: Option<usize>,
    pub incomplete: Option<usize>,
    pub citation: Option<usize>,
    pub implication: Option<usize>,
    pub test: Option<usize>,
    pub exception: Option<usize>,
    pub todo: Option<usize>,
    pub related: Option<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRefStatus {
    pub spec: Option<bool>,
    pub citation: Option<bool>,
    pub implication: Option<bool>,
    pub test: Option<bool>,
    pub exception: Option<bool>,
    pub todo: Option<bool>,
    pub level: Option<String>,
}
```

### Annotation Key Design

**Problem:** Annotation IDs in each report are just array indices (0, 1, 2, ...), so they collide when merging.

**Solution:** Create a stable key based on annotation content:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct AnnotationKey {
    target_path: String,
    target_section: Option<String>,
    source: String,
    line: usize,
}

impl From<&JsonAnnotation> for AnnotationKey {
    fn from(anno: &JsonAnnotation) -> Self {
        Self {
            target_path: anno.target_path.clone(),
            target_section: anno.target_section.clone(),
            source: anno.source.clone(),
            line: anno.line.unwrap_or(0),
        }
    }
}
```

This key uniquely identifies an annotation across all reports since it's based on:
- Which specification section it references
- Which source file it comes from
- Which line in that source file

### Merge Algorithm

#### Phase 1: Collect and Deduplicate Annotations

```
annotation_map: BTreeMap<AnnotationKey, JsonAnnotation> = {}
old_to_key: HashMap<(report_index, old_id), AnnotationKey> = {}

For each report R at index r:
  For each annotation A at index i in R.annotations:
    key = AnnotationKey::from(A)
    
    If key not in annotation_map:
      annotation_map[key] = A
    Else:
      // Annotation already exists from another report
      // Keep the first one (they should be identical)
      pass
    
    old_to_key[(r, i)] = key
```

#### Phase 2: Assign New IDs

```
merged_annotations: Vec<JsonAnnotation> = []
key_to_new_id: HashMap<AnnotationKey, usize> = {}

For each (key, annotation) in annotation_map (sorted):
  new_id = merged_annotations.len()
  merged_annotations.push(annotation)
  key_to_new_id[key] = new_id
```

#### Phase 3: Merge Statuses

```
status_by_key: HashMap<AnnotationKey, JsonStatus> = {}

For each report R at index r:
  For each (old_id_str, status) in R.statuses:
    old_id = parse(old_id_str)
    key = old_to_key[(r, old_id)]
    
    If key not in status_by_key:
      status_by_key[key] = status
    Else:
      // Merge status counts
      status_by_key[key].spec += status.spec
      status_by_key[key].incomplete += status.incomplete
      status_by_key[key].citation += status.citation
      status_by_key[key].implication += status.implication
      status_by_key[key].test += status.test
      status_by_key[key].exception += status.exception
      status_by_key[key].todo += status.todo
      
      // Union related IDs (will remap later)
      status_by_key[key].related = union(existing.related, status.related)
```

#### Phase 4: Remap Status IDs and Related References

```
merged_statuses: HashMap<String, JsonStatus> = {}

For each (key, status) in status_by_key:
  new_id = key_to_new_id[key]
  
  // Remap related annotation IDs
  If status.related is not empty:
    remapped_related = []
    For each old_related_id in status.related:
      For each report R at index r:
        If old_related_id exists in R.annotations:
          related_key = old_to_key[(r, old_related_id)]
          new_related_id = key_to_new_id[related_key]
          remapped_related.push(new_related_id)
    
    status.related = deduplicate(remapped_related)
  
  merged_statuses[new_id.to_string()] = status
```

#### Specifications Merging
```
For each input report:
  For each specification in report:
    If specification not in merged_specs:
      Add specification to merged_specs
    Else:
      Verify specifications are identical (same content)
      If different, log warning but use first version
```

#### Link Conflict Resolution
```
blob_links = collect all non-null blob_link values from input reports
issue_links = collect all non-null issue_link values from input reports

If blob_links is empty:
  merged_blob_link = None
Else if all values in blob_links are identical:
  merged_blob_link = Some(blob_links[0])
Else:
  merged_blob_link = None  // Conflict detected, omit field

(Same logic for issue_link)
```

### Refs Array Handling

The `refs` array is a lookup table for all possible annotation status combinations. Since this is deterministic and doesn't depend on the specific annotations, we can:

**Option 1:** Use the refs array from any input report (they should all be identical)
**Option 2:** Regenerate it (copy the logic from `RefStatus::for_each`)

**Recommendation:** Use Option 1 for simplicity - take the refs array from the first report.

## Implementation Strategy

### Approach: JSON-to-JSON Merge

Given the complexity of reconstructing internal Rust types from JSON, we'll implement a JSON-level merge:

1. **Deserialize** input JSON reports into schema structures
2. **Merge** at the JSON level with annotation ID remapping
3. **Serialize** merged result to JSON
4. **For HTML/LCOV:** Reconstruct minimal internal structures or create new generators

### Why This Approach?

- **Simpler:** Avoids reconstructing complex internal types (Specification, Reference, etc.)
- **Maintainable:** Clear separation between merge logic and existing report generation
- **Extensible:** Easy to add new merge strategies in the future

## Error Handling

### File Not Found
```
Error: Failed to read input file 'packages/foo/report.json'
Caused by: No such file or directory
```

### Invalid JSON
```
Error: Failed to parse JSON from 'packages/foo/report.json'
Caused by: expected value at line 10 column 5
```

### Schema Mismatch
```
Error: Invalid JSON schema in 'packages/foo/report.json'
Caused by: missing required field 'specifications'
```

### Partial Failure Strategy
- Log warning for failed files
- Continue processing remaining files
- Exit with error code if any file failed
- Include summary of successes/failures

## Testing Strategy

### Unit Tests
- Annotation key generation
- Annotation ID remapping
- Link conflict resolution logic
- Status merging logic
- Related ID remapping
- JSON deserialization

### Integration Tests
- Merge 2 simple reports
- Merge reports with conflicting links
- Merge reports with overlapping annotations (same requirement in multiple packages)
- Merge reports with disjoint requirements
- Merge reports with related annotations
- Error handling for invalid inputs

### Property-Based Tests

**Framework:** Use `proptest` or `quickcheck` (check existing test dependencies)

#### Property 1: Annotation Preservation
For all input reports and merged report:
```
merged.annotations.len() >= max(input.annotations.len())
```
(May be less due to deduplication of identical annotations)

#### Property 2: Status Additivity
For annotations with the same key across reports:
```
merged.status[key].citation == sum(report.status[key].citation for all reports)
```

#### Property 3: Merge Commutativity
```
merge([A, B]) ≡ merge([B, A])
```
(Modulo annotation ordering)

#### Property 4: Merge Associativity
```
merge([merge([A, B]), C]) ≡ merge([A, merge([B, C])])
```

## Correctness Properties

### Property 1: Annotation Key Uniqueness
**Validates: Requirements 1.3**

For any annotation A:
```
key(A) = (A.target_path, A.target_section, A.source, A.line)
```
This key uniquely identifies an annotation across all reports.

### Property 2: Status Preservation
**Validates: Requirements 1.3**

For any annotation key K present in reports R₁ and R₂:
```
merged.statuses[new_id(K)].citation == 
  R₁.statuses[old_id₁(K)].citation + R₂.statuses[old_id₂(K)].citation
```

### Property 3: ID Remapping Correctness
**Validates: Requirements 1.3**

For any annotation with old ID `i` in report R:
```
key = (R.annotations[i].target_path, R.annotations[i].target_section, 
       R.annotations[i].source, R.annotations[i].line)
new_id = key_to_new_id[key]
merged.annotations[new_id] ≡ R.annotations[i]
```

### Property 4: Related ID Remapping
**Validates: Requirements 1.3**

For any status with related IDs:
```
For each old_related_id in original_status.related:
  ∃ new_related_id in merged_status.related such that:
    merged.annotations[new_related_id] ≡ original_report.annotations[old_related_id]
```

### Property 5: Link Consistency
**Validates: Requirements 1.3**

```
If all non-null blob_links are identical:
  merged.blob_link == Some(that_value)
Else:
  merged.blob_link == None
```

### Property 6: Specification Completeness
**Validates: Requirements 1.3**

For all specifications S in any input report:
```
S ∈ merged.specifications
```

## File Structure

```
duvet/src/
├── lib.rs                    # Add Merge variant to Arguments enum
├── merge.rs                  # New module (public interface)
├── merge/
│   ├── mod.rs               # Re-exports
│   ├── schema.rs            # JSON deserialization structures
│   ├── logic.rs             # Core merge logic with ID remapping
│   └── tests.rs             # Unit and integration tests
```

## Dependencies

### Existing Dependencies
- `serde` and `serde_json` - JSON parsing (already in project)
- `clap` - Command-line parsing (already in project)
- `tokio` - Async runtime (already in project)

### New Dependencies
None required - all functionality can be implemented with existing dependencies.

## Key Design Decisions

### Decision 1: Annotation Deduplication Strategy

**Problem:** Same annotation might appear in multiple reports if packages share code or specifications.

**Decision:** Deduplicate annotations based on AnnotationKey (target_path, target_section, source, line).

**Rationale:** 
- Prevents duplicate entries in merged report
- Maintains semantic correctness
- Simplifies status merging

### Decision 2: Status Merging Strategy

**Problem:** Annotation IDs are report-local indices, not globally unique.

**Decision:** Use content-based keys for merging, then assign new sequential IDs.

**Rationale:**
- Ensures correct status aggregation across reports
- Maintains compatibility with existing JSON schema
- Allows proper remapping of related annotation references

### Decision 3: Related ID Remapping

**Problem:** The `related` field in statuses contains annotation IDs that need remapping.

**Decision:** Two-pass approach:
1. First pass: Build annotation key mappings
2. Second pass: Remap all related IDs using the mappings

**Rationale:**
- Ensures referential integrity in merged report
- Prevents dangling references
- Maintains semantic relationships between annotations

### Decision 4: HTML/LCOV Generation

**Problem:** Existing generators expect `ReportResult` structure, not JSON.

**Options:**
- A: Reconstruct `ReportResult` from merged JSON
- B: Create new JSON-to-HTML and JSON-to-LCOV generators
- C: Support JSON output only initially

**Decision:** Start with Option C (JSON only), evaluate A vs B for Phase 2.

**Rationale:**
- Delivers core value quickly
- Allows evaluation of reconstruction complexity
- JSON output is the primary use case

## Open Questions

1. **Specification Content Conflicts**: What if two reports have the same specification path but different section content?
   - **Recommendation**: Use first encountered version, log warning

2. **Refs Array**: Should we validate that all input reports have identical refs arrays?
   - **Recommendation**: Yes, validate and fail if different (indicates schema version mismatch)

3. **Download Path**: Not present in JSON output, needed for `ReportResult`. How to handle?
   - **Recommendation**: Use current working directory or make optional for merge operations

4. **Annotation Ordering**: Should merged annotations be sorted?
   - **Recommendation**: Yes, sort by AnnotationKey for deterministic output

## Performance Considerations

- **Parallel File Reading**: Use `tokio::task::JoinSet` to read multiple JSON files concurrently
- **Memory Usage**: For 100 packages @ ~1MB each = ~100MB in memory (acceptable)
- **Merge Complexity**: O(n log n) where n is total annotations (for sorting and deduplication)
- **ID Remapping**: O(n × m) where n = annotations, m = average related count (typically small)

## Backward Compatibility

- No changes to existing commands or JSON schema
- Merged output is compatible with existing report viewers
- No breaking changes to public APIs

## Future Enhancements

1. **Per-Package Metadata**: Extend JSON schema to support package-specific `blob_link` and `issue_link`
2. **Configuration File**: Support merge configuration in `.duvet.toml`
3. **Filtering**: Add flags to filter packages during merge
4. **Incremental Merge**: Cache merged results and only re-merge changed reports
5. **Conflict Resolution Strategies**: Allow users to choose how to handle conflicts
6. **HTML/LCOV Direct Generation**: Implement generators that work directly from merged JSON
