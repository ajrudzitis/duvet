# Rehydrating a `ReportResult` from a JSON v2 Report

## Goal

Reconstruct a `ReportResult` (the internal model used by all report writers) from a serialized `ReportV2` JSON file, without access to the original source files or specification documents.

## Why This Is Interesting

If we can round-trip through JSON v2, we unlock:
- Re-rendering reports in different formats (e.g., read JSON v2, emit snapshot or HTML) without re-scanning sources
- Diffing two reports at the semantic level
- Tooling that consumes reports without needing the Duvet pipeline
- CI checks that operate on a previously-generated report artifact

## The Core Challenge: `Slice`

Almost every type in the internal model depends on `Slice<SourceFile>`:

```
ReportResult
  └─ TargetReport
       ├─ references: Vec<Reference>
       │    └─ text: Slice          ← points into a SourceFile (the spec document)
       ├─ specification: Arc<Specification>
       │    └─ sections[].lines: Vec<Line>
       │         └─ Line::Str(Slice)  ← points into the spec SourceFile
       │    └─ sections[].full_title: Slice
       └─ statuses: StatusMap       ← computed from Reference byte offsets
```

A `Slice<SourceFile>` is:
```rust
struct Slice<File = SourceFile> {
    file: File,       // Arc-wrapped file with path + contents
    start: usize,     // byte offset into file contents
    end: usize,       // byte offset into file contents
}
```

## What the JSON v2 Report Contains (Post Phase 2.5)

The v2 schema stores raw content and byte ranges, not pre-segmented lines:

```
ReportV2
  ├─ repositories: Map<"repo-xxx", { blob_link }>
  ├─ sources
  │    ├─ inline: Map<"src-xxx", { file_name, contents }>     ← full spec file text
  │    └─ linked: Map<"lnk-xxx", { file_name, repository? }>  ← source code metadata
  ├─ annotations
  │    ├─ specification: Map<"spc-xxx", { source: SourceRef, title?, format }>
  │    ├─ section: Map<"spc-xxx", { source: SourceRef, short_name, long_name? }>
  │    ├─ requirement: Map<"req-xxx", { source: SourceLocation, origin: SourceRef, level, coverage: Map<cite-ID, [ByteRange]> }>
  │    └─ impl: Map<"cite-xxx", { source: SourceLocation, target_source, target_ranges: [ByteRange], type, ... }>
  └─ issue_links: [String]
```

Key observations:
1. **Full spec file contents are present** in `sources.inline` — no need to synthesize from section lines.
2. **All byte ranges are explicit** — `SourceRef` has `start`/`end`, `ByteRange` has `start`/`end`, `ImplAnnotation` has `target_ranges`.
3. **Coverage is byte-range-based** — `RequirementAnnotation.coverage` maps cite-IDs to `Vec<ByteRange>`, preserving exact coverage attribution.
4. **No pre-segmented lines** — segmentation must be recomputed from byte ranges (needed for v1 conversion or frontend rendering).

## Approach: Direct `SourceFile` from Inline Contents

### The Key Insight

Unlike the old v2 schema (which stored pre-segmented lines and required synthetic file reconstruction), the new schema stores **full file contents** in `InlineSource.contents`. We can create a `SourceFile` directly from this content, and all byte offsets in the report are already absolute offsets into this content.

### Step-by-Step Reconstruction

#### Step 1: Create `SourceFile` per inline source

```rust
fn create_source_files(sources: &BTreeMap<String, InlineSource>) -> HashMap<String, SourceFile> {
    sources.iter().map(|(id, src)| {
        let file = SourceFile::new(Path::from(&src.file_name), &src.contents).unwrap();
        (id.clone(), file)
    }).collect()
}
```

All `SourceRef` byte ranges in the report are valid offsets into these files.

#### Step 2: Rebuild `Specification` and `Section`s

For each specification annotation, find its sections by byte range containment:

```rust
fn rebuild_specification(
    spec_anno: &SpecificationAnnotation,
    section_annos: &[(String, &SectionAnnotation)],  // filtered to same source
    source_file: &SourceFile,
) -> Specification {
    let mut sections = HashMap::new();

    for (_id, section) in section_annos {
        let full_title = source_file.substr_range(section.source.start..section.source.end).unwrap();

        // Extract lines from the section's byte range in the source file
        let section_text = &source_file[section.source.start..section.source.end];
        let lines: Vec<Line> = source_file.lines_slices()
            .filter(|slice| {
                let r = slice.range();
                r.start >= section.source.start && r.end <= section.source.end
            })
            .map(Line::Str)
            .collect();

        sections.insert(section.short_name.clone(), Section {
            id: section.short_name.clone(),
            title: section.long_name.clone().unwrap_or_default(),
            full_title,
            lines,
        });
    }

    Specification {
        title: spec_anno.title.clone(),
        sections,
        format: spec_anno.format.parse().unwrap_or_default(),
    }
}
```

#### Step 3: Rebuild `Annotation`s and `Reference`s

Each `ImplAnnotation` maps to an `Annotation` + one or more `Reference`s. The `target_ranges` directly give us the byte ranges for `Reference.text`:

```rust
// For each ImplAnnotation:
for range in &impl_anno.target_ranges {
    let text = spec_file.substr_range(range.start..range.end).unwrap();
    references.push(Reference {
        target: target.clone(),
        text,
        annotation: annotation_with_id.clone(),
    });
}
```

Each `RequirementAnnotation` maps to a SPEC-type `Annotation` + `Reference`:

```rust
let text = spec_file.substr_range(req.origin.start..req.origin.end).unwrap();
references.push(Reference {
    target: target.clone(),
    text,
    annotation: spec_annotation_with_id.clone(),
});
```

#### Step 4: Rebuild `StatusMap`

Call `StatusMap::populate()` on the rebuilt references. This recomputes coverage from byte offsets, validating the reconstruction. The `populate()` method only needs `Reference.start()`, `Reference.end()`, `Reference.annotation.id`, and `Reference.annotation.anno`.

Alternatively, coverage can be reconstructed directly from `RequirementAnnotation.coverage` byte ranges, which is more efficient and preserves the exact per-annotation attribution.

#### Step 5: Assemble `ReportResult`

```rust
fn rehydrate(report_v2: &ReportV2) -> ReportResult<'static> {
    // 1. Create SourceFiles from inline sources
    // 2. Rebuild Specifications from spec + section annotations
    // 3. Rebuild Annotations from impl + requirement annotations
    // 4. Rebuild References from target_ranges and coverage byte ranges
    // 5. Compute StatusMap from references
    // 6. Assemble ReportResult
}
```

## Data Gaps in JSON v2

| Data needed by consumers | Present in JSON v2? | Workaround |
|---|---|---|
| Spec file contents | ✅ `InlineSource.contents` | — |
| Section id, title, byte range | ✅ `SectionAnnotation` | — |
| Spec format (ietf/markdown) | ✅ `SpecificationAnnotation.format` | — |
| Spec title | ✅ `SpecificationAnnotation.title` | — |
| Annotation type, level, quote | ✅ (type/level explicit, quote recoverable from `target_ranges` + contents) | — |
| Annotation source path | ✅ `LinkedSource.file_name` | — |
| Annotation line number | ✅ `SourceLocation.line` | — |
| Annotation comment/feature/tracking_issue/tags | ✅ `ImplAnnotation` fields | — |
| Byte offsets of references in spec | ✅ `target_ranges` and `coverage` byte ranges | — |
| `blob_link` per annotation | ✅ Via `LinkedSource.repository` → `Repository.blob_link` | — |
| `original_target` / `original_text` / `original_quote` Slices | ❌ Only string values | Synthesize from annotation content |
| `download_path` | ❌ Not serialized | Not needed for re-rendering; use a placeholder |
| `require_citations` / `require_tests` | ❌ Not serialized | **Must be added to JSON v2 or passed separately** |
| `manifest_dir` on Annotation | ❌ Not serialized | Use source path's parent as approximation |

### Critical Gap: `require_citations` and `require_tests`

These booleans on `TargetReport` control CI enforcement behavior. They're set from CLI flags / config and are not in the JSON v2 output. If rehydration is meant to support CI checks, these need to either:
- Be added to the JSON v2 format (per-specification or globally)
- Be passed as parameters to the rehydration function

### Minor Gap: `download_path`

`ReportResult` holds a `download_path: &Path`. For rehydration, this is irrelevant (we're not loading specs from disk). Either change `ReportResult` to use owned types, or use a static placeholder.

## Advantages Over Old v2 Schema

The new v2 schema (Phase 2.5) dramatically simplifies rehydration compared to the old schema:

| Aspect | Old v2 (pre-Phase 2.5) | New v2 (post-Phase 2.5) |
|---|---|---|
| Spec file contents | Reconstructed from concatenated section lines | Stored directly in `InlineSource.contents` |
| Byte offsets | Implicit (computed from segment text lengths) | Explicit (`SourceRef`, `ByteRange`) |
| Coverage | Byte counts only (`CoverageStatus.citation: usize`) | Byte ranges per annotation (`coverage: Map<cite-ID, Vec<ByteRange>>`) |
| Segmentation | Pre-computed (invalid after merge) | Not stored (computed on demand from byte ranges) |
| Line numbers | Not stored | Not needed — `SourceFile` computes them from content |

## Validation Strategy

Round-trip test: generate a `ReportResult` from the normal pipeline, serialize to `ReportV2`, rehydrate back, then re-serialize to `ReportV2` and assert equality:

```rust
#[test]
fn round_trip() {
    let original_report: ReportResult = /* from normal pipeline */;
    let json_v2 = ReportV2::from_report_result(&original_report);
    let rehydrated = json_v2.to_report_result();
    let re_serialized = ReportV2::from_report_result(&rehydrated);
    assert_eq!(json_v2, re_serialized);
}
```

## Open Questions

1. **Should `require_citations`/`require_tests` be added to the JSON v2 schema?** Needed for CI enforcement from a rehydrated report.

2. **Is the `ReportResult<'a>` lifetime a blocker?** The `'a` comes from `blob_link: Option<&'a str>`, `issue_link: Option<&'a str>`, and `download_path: &'a Path`. For rehydration, these need to be owned. The cleanest fix is changing these three fields to owned types (or `Cow`).

3. **How should annotations that failed matching be handled?** Impl annotations with empty `target_ranges` had no match. These should still be included in the rehydrated `AnnotationSet` but won't produce `Reference`s — which is correct behavior.
