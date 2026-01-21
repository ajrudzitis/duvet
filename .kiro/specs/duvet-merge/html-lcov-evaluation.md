# HTML/LCOV Generation Approach Evaluation

## Task 15.1: Complexity Assessment

### Current State Analysis

#### What HTML Generation Requires

The existing `html::report()` function is remarkably simple:
```rust
pub fn report(report: &ReportResult, file: &Path) -> Result {
    // Creates HTML wrapper
    // Embeds JSON data in <script> tag
    // Includes JavaScript viewer
}
```

**Key insight:** HTML generation is just a thin wrapper around JSON! The actual rendering happens client-side in JavaScript.

**Dependencies:**
- `ReportResult` structure (only for passing to `json::report_writer()`)
- JSON serialization (already implemented)
- Static JavaScript viewer code (already bundled)

#### What LCOV Generation Requires

The `lcov::report()` function is more complex and requires:

**Critical Dependencies:**
1. `ReportResult` with:
   - `targets: BTreeMap<Arc<Target>, TargetReport>`
   - `download_path: &Path`
   
2. `TargetReport` with:
   - `references: Vec<Reference>` - **COMPLEX**
   - `specification: Arc<Specification>` - **COMPLEX**
   - `require_citations: bool`
   - `require_tests: bool`

3. `Reference` structure:
   - `target: Arc<Target>`
   - `annotation: AnnotationWithId`
   - `text: Slice` - **REQUIRES SOURCE FILE PARSING**

4. `Specification` structure:
   - `sections: HashMap<String, Section>`
   - Each `Section` has `lines: Vec<Line>` with `Slice` references

**Complexity Factors:**
- `Slice` is a reference to actual file content with byte ranges
- `Reference` requires matching annotations to specification text
- `Specification` requires parsing the original spec files
- LCOV needs line-by-line coverage mapping

### Reconstruction Complexity Analysis

#### Option A: Reconstruct ReportResult from Merged JSON

**For HTML:**
- **Complexity: LOW** ✅
- HTML doesn't actually need `ReportResult` - it just needs JSON
- We can bypass `ReportResult` entirely and write JSON directly to HTML

**For LCOV:**
- **Complexity: VERY HIGH** ❌
- Would need to:
  1. Parse all specification files again to get `Slice` references
  2. Parse all source files to get annotation locations
  3. Reconstruct `Reference` objects with text matching
  4. Rebuild `Specification` structures with sections
  5. Create `Target` objects with proper paths
  6. Handle URL downloads for remote specifications
  
**Estimated Effort:** 
- HTML: 1-2 hours (simple wrapper modification)
- LCOV: 20-40 hours (essentially reimplementing the entire report pipeline)

#### Option B: Create New Generators from Merged JSON

**For HTML:**
- **Complexity: VERY LOW** ✅
- Just modify existing HTML generator to accept JSON directly
- No reconstruction needed

**For LCOV:**
- **Complexity: HIGH** ⚠️
- Would need to:
  1. Design new LCOV format that works without `Slice` references
  2. Implement line-by-line mapping from JSON section data
  3. Handle coverage calculations without `Reference` objects
  4. May lose some fidelity compared to original LCOV output
  
**Estimated Effort:**
- HTML: 1 hour (trivial modification)
- LCOV: 10-15 hours (new generator design and implementation)

#### Option C: JSON-Only Initially

**Complexity: ZERO** ✅ (Already implemented)

**Pros:**
- Delivers core value immediately
- JSON is the primary use case for merging
- Users can still view merged JSON in HTML by running `duvet report` on merged data
- No additional implementation needed

**Cons:**
- Users can't directly generate HTML/LCOV from merged reports
- Requires two-step process: merge → report

### Key Findings

1. **HTML is trivial** - it's just a JSON wrapper with JavaScript
2. **LCOV is extremely complex** - requires full specification and source parsing
3. **The merged JSON contains all the data** - but not in the format LCOV needs
4. **Reconstruction is not practical** - would require reimplementing the entire report pipeline

## Task 15.2: Decision Documentation

### Decision: Hybrid Approach

**Immediate (Phase 6):**
- ✅ **HTML: Implement** using Option B (new generator from JSON)
- ❌ **LCOV: Defer** to future enhancement

**Rationale:**

#### Why Implement HTML Now:
1. **Trivial effort** - 1-2 hours of work
2. **High value** - Users want visual reports
3. **No complexity** - Just modify wrapper to accept JSON directly
4. **Maintains compatibility** - Same HTML output as before

#### Why Defer LCOV:
1. **High complexity** - 10-40 hours depending on approach
2. **Lower priority** - LCOV is less commonly used than HTML
3. **Workaround exists** - Users can generate LCOV before merging
4. **Unclear requirements** - Need user feedback on LCOV use cases for merged reports

### Implementation Plan for HTML

**Approach:** Modify HTML generator to accept merged JSON directly

```rust
// New function in duvet/src/merge/mod.rs
async fn write_html_output(&self, merged_report: &schema::MergedReport, output_path: &Path) -> Result {
    // Serialize merged report to JSON string
    let json_data = serde_json::to_string(merged_report)?;
    
    // Write HTML wrapper with embedded JSON
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
    
    std::fs::write(output_path, html)?;
    Ok(())
}
```

**Estimated Time:** 1-2 hours
**Risk:** Very low
**Testing:** Verify HTML opens in browser and displays merged data correctly

### Future Work for LCOV

**Recommended Approach:** Option B (New Generator)

**Why:**
- Reconstruction (Option A) is too complex and fragile
- New generator can be optimized for merged JSON structure
- Can iterate on design based on user feedback

**Requirements Gathering Needed:**
1. What coverage metrics are most important for merged reports?
2. How should line-level coverage work across multiple packages?
3. Should LCOV show per-package breakdown or unified view?
4. What's the expected workflow for using LCOV with merged reports?

**Estimated Effort:** 10-15 hours once requirements are clear

## Task 15.3: Implementation Plan

### Phase 6 Updated Plan

#### Task 16: Implement HTML Output (APPROVED)

**Subtasks:**
- 16.1 Create `write_html_output()` method in `duvet/src/merge/mod.rs`
- 16.2 Modify `exec()` to call `write_html_output()` when `--html` flag is provided
- 16.3 Remove "not yet implemented" warning for HTML
- 16.4 Add integration test for HTML output with 2 input files
- 16.5 Add integration test for HTML output with 3 input files
- 16.6 Manually verify HTML opens in browser and displays correctly

**Acceptance Criteria:**
- HTML file is generated with proper structure
- JSON data is embedded in script tag
- JavaScript viewer is included
- HTML opens in browser without errors
- Merged data displays correctly in viewer
- Integration tests pass

**Estimated Time:** 1-2 hours

#### Task 17: LCOV Output (DEFERRED)

**Status:** Deferred to future enhancement

**Reason:** High complexity (10-15 hours) with unclear requirements and lower priority

**Recommended Next Steps:**
1. Gather user feedback on LCOV use cases for merged reports
2. Design LCOV format for merged multi-package data
3. Implement new LCOV generator based on requirements
4. Add comprehensive testing

**Workaround:** Users can generate LCOV reports before merging, or run `duvet report --lcov` on individual packages

### Updated Phase 6 Tasks

```markdown
## Phase 6: HTML Output

- [ ] 16. Implement HTML output from merged JSON
  - [ ] 16.1 Create `write_html_output()` method in `duvet/src/merge/mod.rs`
  - [ ] 16.2 Modify `exec()` to call `write_html_output()` when `--html` flag is provided
  - [ ] 16.3 Remove "not yet implemented" warning for HTML
  - [ ] 16.4 Add integration test for HTML output with 2 input files
  - [ ] 16.5 Add integration test for HTML output with 3 input files
  - [ ] 16.6 Manually verify HTML opens in browser and displays correctly

- [ ]* 17. LCOV Output (DEFERRED - Future Enhancement)
  - [ ]* 17.1 Gather user requirements for LCOV with merged reports
  - [ ]* 17.2 Design LCOV format for multi-package merged data
  - [ ]* 17.3 Implement new LCOV generator from merged JSON
  - [ ]* 17.4 Add integration tests for LCOV output
  - [ ]* 17.5 Document LCOV limitations and workarounds
```

### Summary

**Decision:** Implement HTML now (trivial), defer LCOV (complex)

**HTML Implementation:**
- Approach: Direct JSON-to-HTML wrapper (no ReportResult reconstruction)
- Effort: 1-2 hours
- Risk: Very low
- Value: High (users want visual reports)

**LCOV Deferral:**
- Reason: High complexity with unclear requirements
- Workaround: Generate LCOV before merging
- Future: Implement after gathering user feedback

**Next Steps:**
1. Update tasks.md with revised Phase 6 plan
2. Implement Task 16 (HTML output)
3. Mark Task 17 as optional/deferred
4. Document LCOV workaround in README

