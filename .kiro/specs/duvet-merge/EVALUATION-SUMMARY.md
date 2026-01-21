# HTML/LCOV Evaluation Summary

## Decision: Hybrid Approach

✅ **HTML: Implement Now** - Trivial wrapper around JSON (1-2 hours)  
❌ **LCOV: Defer** - High complexity with unclear requirements (10-15 hours)

## Key Findings

### HTML Generation
- **Current implementation:** Just embeds JSON in HTML with JavaScript viewer
- **Complexity:** VERY LOW - no ReportResult reconstruction needed
- **Approach:** Write JSON directly to HTML template
- **Effort:** 1-2 hours
- **Value:** HIGH - users want visual reports

### LCOV Generation
- **Current implementation:** Requires full ReportResult with:
  - Parsed specifications with Slice references
  - Reference objects with text matching
  - Source file parsing for line-level coverage
- **Complexity:** VERY HIGH - would need to reimplement report pipeline
- **Reconstruction effort:** 20-40 hours
- **New generator effort:** 10-15 hours
- **Value:** MEDIUM - less commonly used, workaround exists

## Implementation Plan

### Immediate: Task 16 (HTML Output)

```rust
// Simple implementation in duvet/src/merge/mod.rs
async fn write_html_output(&self, merged_report: &schema::MergedReport, output_path: &Path) -> Result {
    let json_data = serde_json::to_string(merged_report)?;
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

**Testing:**
- Integration test with 2 input files
- Integration test with 3 input files
- Manual browser verification

### Deferred: Task 17 (LCOV Output)

**Reason for Deferral:**
- High implementation complexity
- Unclear user requirements for merged LCOV
- Workaround exists (generate LCOV before merging)
- Lower priority than HTML

**Before Implementation:**
1. Gather user feedback on LCOV use cases
2. Design LCOV format for multi-package data
3. Decide on coverage aggregation strategy

**Workaround:**
Users can generate LCOV reports for individual packages before merging:
```bash
# Generate LCOV for each package
cd package1 && duvet report --lcov report.lcov
cd package2 && duvet report --lcov report.lcov

# Then merge JSON reports
duvet merge package1/report.json package2/report.json --json merged.json --html merged.html
```

## Next Steps

1. ✅ Update tasks.md with revised Phase 6 plan (DONE)
2. Implement Task 16 (HTML output)
3. Update documentation to explain LCOV workaround
4. Consider adding note in CLI help about LCOV deferral

## References

- Full evaluation: `.kiro/specs/duvet-merge/html-lcov-evaluation.md`
- Existing HTML generator: `duvet/src/report/html.rs`
- Existing LCOV generator: `duvet/src/report/lcov.rs`
- Merge implementation: `duvet/src/merge/`
