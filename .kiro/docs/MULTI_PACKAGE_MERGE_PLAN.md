# Multi-Package Report Merge Plan

## Problem Statement

A single specification is implemented across multiple packages. Due to build system constraints, duvet runs in isolation per package. The goal is a unified "single pane of glass" report tracking conformance across all packages.

## Current Limitations

1. ~~**Single blob_link**: `HtmlReport.blob_link` is one string. Merged reports need per-package links since source files live in different repos/paths.~~ ✅ **RESOLVED** - Per-source `blob-link` now supported in `[[source]]` config blocks, propagated to annotations.

2. **JSON format is write-only**: `json.rs` uses streaming macros with no deserialization. Format is optimized for React frontend, not roundtripping.

3. **Ephemeral annotation IDs**: IDs assigned sequentially during `reference_map()`. Merging causes collisions. IDs don't need to be stable across runs (any code change regenerates the report), but must be deterministic from content so two independent report runs produce the same ID for the same annotation.

4. **No package context**: Annotations track `source` (file path) but not which package. Can't distinguish `src/lib.rs` from package A vs B after merge.

---

## Phase 1: Data Model Enhancements

### 1.1 Support blob links via source config ✅ COMPLETED (commit 75acb7c4)

Extend `[[source]]` blocks to accept `blob-link`:

```toml
[[source]]
pattern = "src/**/*.rs"
blob-link = "https://github.com/org/my-package/blob/main"  # optional, overrides report.html.blob-link
```

Config schema changes (`v0_4_0.rs`):
```rust
pub struct Source {
    pub pattern: String,
    pub blob_link: Option<String>,    // NEW
    // existing fields...
}
```

Internal `Source` struct (`config.rs`):
```rust
pub struct Source {
    pub pattern: String,
    pub root: Path,
    pub comment_style: crate::comment::Pattern,
    pub default_type: crate::annotation::AnnotationType,
    pub blob_link: Option<Arc<str>>,  // NEW
}
```

Resolution logic:
1. Annotation inherits `blob_link` from its source config
2. If source has no `blob-link`, fall back to `report.html.blob-link`

This keeps per-package config self-contained in each package's `.duvet/config.toml`.

**Implementation notes:**
- `blob_link` field added to `Annotation` struct (`annotation.rs`)
- Comment parser propagates `blob_link` from source config to parsed annotations
- JSON report includes `blob_link` per annotation when present
- Frontend (`www/src/result.js`) uses annotation's `blob_link` if present, falls back to global

### 1.2 Cite-style stable annotation IDs ✅ COMPLETED

Replace sequential `usize` IDs with content-based deterministic IDs for developer-authored annotations (citations, tests, etc.).

**Composite key:**
```
(source_path, anno_line, resolved_target_path) → deterministic ID
```

`(source_path, anno_line)` is already unique within a package — two annotations can't start on the same line in the same file. Including `resolved_target_path` is cheap insurance and aids debugging.

Fields deliberately excluded:
- `target_section`, `anno_type`, `quote_hash`: redundant given `(source, line)` uniqueness
- `quote`: can be long; not needed for uniqueness

**Stability guarantee:** IDs are deterministic from content, not stable across code changes. Any source edit regenerates the report. The only requirement is that two independent report runs over the *same* source produce the same IDs, so that merge can union them.

**ID function (implemented in `annotation.rs`):**

```rust
fn stable_annotation_id(annotation: &Annotation) -> String {
    let mut buf = Vec::new();
    let _ = write!(buf, "{}\0{}\0{}",
        annotation.source.to_string_lossy(),
        annotation.anno_line,
        annotation.resolve_target_path(),
    );
    format!("{:016x}", fnv1a_64(&buf))
}
```

Produces a 16-char hex string (e.g., `"a3f7b2c1e9d04856"`). FNV-1a is deterministic, has no dependencies, and 64 bits is more than sufficient for the expected annotation counts.

**Phase-in strategy:**

The v1 JSON frontend indexes annotations by array position (`input.annotations[id]`) and ignores unknown fields. `stable_id` was added as inert metadata without breaking compatibility:

1. `stable_id: String` added to `AnnotationWithId`, computed during `reference_map()`
2. v2 JSON uses `stable_id` as annotation keys
3. Frontend continues using integer indices — `stable_id` is ignored in v1

```rust
pub struct AnnotationWithId {
    pub id: usize,
    pub stable_id: String,  // content-derived, for v2/merge
    pub annotation: Arc<Annotation>,
}
```

**Note:** This ID scheme covers only developer-authored annotations (what will become `cite-` prefixed IDs in Phase 1.3). The v2 schema requires additional ID types for sources, repositories, and spec-derived annotations. Phase 1.3 defines the complete entity-typed ID system.

### 1.3 Entity-typed deterministic IDs ✅ COMPLETED

The v2 schema (Phase 2.5) requires deterministic IDs for all entity types, not just developer-authored annotations. Each entity type has a distinct prefix indicating its hash algorithm.

All IDs use FNV-1a 64-bit → 16-char hex. The prefix indicates the hash input schema — each unique prefix corresponds to exactly one algorithm for computing the hash:

| Prefix | Entity | Hash input | Example |
|--------|--------|------------|---------|
| `src-` | Inline source | `contents` (raw file bytes) | `src-a3f7b2c1e9d04856` |
| `lnk-` | Linked source | `file_name \0 repository_id` | `lnk-b4c8d3e2f1a05967` |
| `repo-` | Repository | `blob_link` | `repo-d6e7f8a9b0c12345` |
| `spc-` | Specification annotation | `source_id \0 start \0 end` (byte range, decimal strings) | `spc-a1b2c3d4e5f60001` |
| `spc-` | Section annotation | `source_id \0 start \0 end` (byte range, decimal strings) | `spc-e5f6a7b8c9d01234` |
| `req-` | Requirement annotation | `origin_id \0 s1 \0 e1 \0 s2 \0 e2 ... \0 source_id \0 line` (decimal strings, ranges sorted ascending) | `req-f7a3b2c1e9d04856` |
| `cite-` | Impl annotation | `source_id \0 line \0 target_source_id` | `cite-c3d4e5f6a7b80912` |

`\0` is the null byte separator between fields. Specification and section annotations share the `spc-` prefix because they use the same algorithm (hash of inline source reference + byte range). For specification annotations, the byte range spans the entire file (start: 0, end: file length). Requirement annotations use the `req-` prefix because their hash additionally includes the authoring site (`lnk-` ID + line) — this distinguishes independent requirements that quote the same spec byte range (e.g., a hand-authored TOML entry and an auto-extracted one, or duplicates in different requirement files).

**Requirement range lists:** A single logical authoring site (one TOML `[[spec]]` or one inline `//= type=spec` comment) may match N disjoint byte ranges in the spec file — this happens when the quote spans regions the spec parser normalized away (IETF RFC page breaks, blank lines, etc.). The `req-` hash includes the full sorted range list so that one logical requirement produces exactly one `req-` ID, regardless of how many fragments the matching process yielded. The hash function sorts the range input internally to make the ID invariant under caller ordering.

**`lnk-` hash when no repository:** When `repository` is `None` (no blob link configured for the source), use an empty string as the repository_id in the hash input: `FNV-1a(file_name \0 "")`. All files without an explicit repository are treated as belonging to the same implicit default repository. This is deterministic and produces consistent IDs across runs.

**Change from Phase 1.2:** The `cite-` hash input changes from `(source_path, anno_line, resolved_target_path)` to `(source_id, line, target_source_id)`. The key difference is `source_id` (the `lnk-xxxx` ID of the linked source) instead of `source_path` (a raw file path string), and `target_source_id` (a `src-xxxx` hash of the spec file contents) instead of `resolved_target_path` (a URL/path string). This decouples annotation identity from the spec's URL — two packages referencing the same spec content via different paths produce the same `target_source_id`. The `source_id` couples annotation identity to the repository (since `lnk-` includes repository_id), which is acceptable — changing a repository URL changes the linked source identity. The existing `stable_annotation_id` function will be superseded by the `cite-` prefixed variant.

**Data availability analysis:**

For each ID type, the hash inputs must be available both at report generation time (from parsed data) and recoverable from the report JSON (for re-identification during merge):

| Prefix | At generation time | From report JSON |
|--------|-------------------|-----------------|
| `src-` | ✅ Spec file contents read from disk | ✅ `InlineSource.contents` |
| `lnk-` | ✅ File path known, repo ID computed from blob_link | ✅ `LinkedSource.file_name` + `LinkedSource.repository` |
| `repo-` | ✅ blob_link from config (`[[source]]` or global) | ✅ `Repository.blob_link` |
| `spc-` spec | ✅ Inline source ID + full file byte range | ✅ `SpecificationAnnotation.source` (all three fields) |
| `spc-` section | ✅ Inline source ID + section byte offsets from parser | ✅ `SectionAnnotation.source` (all three fields) |
| `req-` requirement | ✅ Inline source ID + sorted requirement byte range list + authoring `lnk-` ID + anno_line | ✅ `RequirementAnnotation.origin.src` + `.origin.ranges` + `.source.src` + `.source.line` |
| `cite-` | ✅ Linked source ID from file_name + repo_id, line from annotation parser, inline source ID from spec matching | ✅ `ImplAnnotation.source.src` (the `lnk-xxxx` key) + `.source.line` + `.target.src` |

All inputs are available in both contexts.

**Implementation:** This phase can be implemented and tested independently of Phase 2.5 — it's just hash functions with unit tests against known vectors. The functions are wired into the schema during Phase 2.5.

**Function signatures:** All ID functions take pre-resolved string inputs, not raw `Annotation` structs. For example:

- `repo_id(blob_link: &str) -> String`
- `src_id(contents: &[u8]) -> String`
- `lnk_id(file_name: &str, repository_id: &str) -> String`
- `spc_id(source_id: &str, start: usize, end: usize) -> String`
- `req_id(origin_id: &str, ranges: &[(usize, usize)], source_id: &str, line: usize) -> String`
- `cite_id(source_id: &str, line: usize, target_source_id: &str) -> String`

The caller is responsible for building lookup tables that map entities to their resolved IDs (e.g., mapping each unique blob_link to its `repo-` ID, each spec file's contents to its `src-` ID, etc.). This keeps the hash functions pure and independently testable. The wiring that builds these lookup tables and calls the ID functions with resolved inputs happens in Phase 2.5's `from_report_result()`.

---

## Phase 2: Roundtrip-Friendly JSON Format ✅ COMPLETED (initial implementation)

### 2.1 Define v2 JSON schema ✅

Implemented in `duvet/src/report/json_v2.rs` using serde derive. Resolved open design questions:

- **Bitmap status** chosen over refs table: `u8` bitmask with bits 0-5 for coverage flags, bits 6-7 for level. Enables fast bitwise merge (`a | b`), no lookup table needed.
- **Top-level coverage** chosen over inline: flat `BTreeMap<String, CoverageStatus>` keyed by stable annotation ID.

Current v2 structure:

```rust
pub struct ReportV2 {
    pub version: String,  // "2.0"
    pub blob_link: Option<String>,
    pub issue_link: Option<String>,
    pub specifications: BTreeMap<String, SpecificationV2>,
    pub annotations: Vec<AnnotationV2>,
    pub coverage: BTreeMap<String, CoverageStatus>,
}
```

With `SpecificationV2` containing sections with pre-segmented lines (`LineV2::Plain` or `LineV2::Segmented` with `LineSegmentV2` carrying `annotation_ids`, `status: u8`, `text`), and `AnnotationV2` carrying stable hex IDs, quotes, and all annotation metadata.

### 2.2 Implement read/write for v2 ✅

- CLI: `duvet report --json-v2 <path>` and `DUVET_INTERNAL_CI_JSON_V2` env var
- `ReportV2::from_report_result()` builds v2 from internal `ReportResult`
- `write_report_v2()` / `read_report_v2()` with version validation
- `segment_line()` splits spec lines at annotation boundaries using byte offsets
- Roundtrip tests, segmentation property tests, merge property tests (commutativity, associativity)

### 2.3 Keep existing JSON format ✅

`json.rs` (v1) remains unchanged for HTML frontend compatibility.

### 2.4 v1 vs v2 comparison

> **Note:** This table describes the intermediate v2 format (Phase 2). Phase 2.5 replaces this schema entirely — see "Phase 2.5: v2 Schema Revision" below.

| Aspect | v1 | v2 (current) |
|--------|----|----|
| **Serialization** | Streaming macros, write-only | Serde derive, read/write |
| **Annotation IDs** | Sequential `usize` (ephemeral) | Content-derived 16-char hex (FNV-1a of source_path + anno_line + target_path) |
| **Line segment status** | `status_id` index into 256-entry `refs` table | `status: u8` bitmap (self-contained) |
| **Coverage** | `statuses` map with integer keys | `coverage` map with stable string keys |
| **Blob links** | Per-annotation (from source config) | Per-annotation (same) |
| **Version field** | None | `"2.0"` |

---

## Phase 2.5: v2 Schema Revision ✅ COMPLETED

> **This phase replaces the Phase 2 v2 schema entirely.** All structs from Phase 2 (`ReportV2`, `SpecificationV2`, `SectionV2`, `LineV2`, `LineSegmentV2`, `AnnotationV2`, `CoverageStatus`) are removed and replaced. The `json_v2.rs` module is rewritten.

The initial v2 implementation (Phase 2) is a working roundtrip-friendly format. This phase restructures the schema based on PR review feedback to better support merge semantics and eliminate redundancy.

### Design principles

- **All metadata about content is encompassed in annotations** which contain the metadata, the source, and the byte range or line number over which it applies. Sources are raw content storage; annotations carry all semantic meaning.
- **Coverage byte ranges are strictly richer than byte counts.** From ranges you can compute counts, reconstruct exact segmentation, and attribute coverage per-annotation. Byte counts are lossy. Ranges are essential for correct merge (union of ranges from multiple packages).
- **Pre-computed segmentation is intentionally removed.** Segmentation depends on which annotations exist. After merge, new annotations from other packages change segment boundaries, so pre-computed segmentation is invalid and MUST be recomputed. Storing raw text + byte ranges is the correct primitive for merge.

### Top-level structure

```rust
pub struct ReportV2 {
    pub version: String,                              // "2.0"
    pub issue_links: Vec<String>,
    pub repositories: BTreeMap<String, Repository>,   // "repo-xxxx" keys
    pub sources: SourcesV2,                           // grouped by type
    pub annotations: AnnotationsV2,                   // grouped by type
}
```

### Sources

```rust
pub struct SourcesV2 {
    /// Inline sources (specification files) keyed by "src-xxxx"
    /// JSON key: "https://awslabs.github.io/duvet/v2/sources.json#inline"
    pub inline: BTreeMap<String, InlineSource>,
    /// Linked sources (source code files) keyed by "lnk-xxxx"
    /// JSON key: "https://awslabs.github.io/duvet/v2/sources.json#linked"
    pub linked: BTreeMap<String, LinkedSource>,
}

pub struct Repository {
    pub blob_link: String,
}

pub struct InlineSource {
    pub file_name: String,      // e.g., "rfc9000.txt"
    pub contents: String,       // full file text
}

pub struct LinkedSource {
    pub file_name: String,          // e.g., "src/server.rs"
    pub repository: Option<String>, // "repo-xxxx" key
}
```

**Inline sources** (`#inline`): Full file contents for specification documents. Key is `src-` + FNV-1a of file contents. Identical specs from different packages deduplicate by hash. No metadata fields — title, format, and section titles are captured by annotation types (specification and section annotations) following the principle that all metadata about content is encompassed in annotations.

**Linked sources** (`#linked`): Lightweight metadata for source code files. Key is `lnk-` + FNV-1a of file path + repository ID.

**Repositories**: Top-level map keyed by `repo-` + FNV-1a of blob link. Linked sources reference a repository by ID instead of carrying `blob_link` directly. This deduplicates blob links when many source files share the same repository. If the config does not provide a per-source blob link, the global `report.html.blob-link` is used — but in the report JSON, it is always persisted at the repository level (no top-level `blob_link` field).

### Annotations

```rust
pub struct AnnotationsV2 {
    /// JSON key: "https://awslabs.github.io/duvet/v2/annotations.json#specification"
    pub specification: BTreeMap<String, SpecificationAnnotation>,  // "spc-xxxx"
    /// JSON key: "https://awslabs.github.io/duvet/v2/annotations.json#section"
    pub section: BTreeMap<String, SectionAnnotation>,              // "spc-xxxx"
    /// JSON key: "https://awslabs.github.io/duvet/v2/annotations.json#requirement"
    pub requirement: BTreeMap<String, RequirementAnnotation>,      // "req-xxxx"
    /// JSON key: "https://awslabs.github.io/duvet/v2/annotations.json#impl"
    pub r#impl: BTreeMap<String, ImplAnnotation>,                  // "cite-xxxx"
}
```

Specification and section annotations use the `spc-` prefix — they share the same hashing algorithm (`FNV-1a(source_id \0 start \0 end)`). Requirement annotations use the `req-` prefix (same algorithm extended with authoring `lnk-` ID and line to distinguish duplicates). Impl annotations use the `cite-` prefix. The annotation's type is determined by which group it belongs to, not by the prefix.

**Serde rename attributes:** The `SourcesV2` and `AnnotationsV2` structs require `#[serde(rename = "...")]` attributes on each field to produce the schema URL keys in JSON. For example:

```rust
pub struct SourcesV2 {
    #[serde(rename = "https://awslabs.github.io/duvet/v2/sources.json#inline")]
    pub inline: BTreeMap<String, InlineSource>,
    #[serde(rename = "https://awslabs.github.io/duvet/v2/sources.json#linked")]
    pub linked: BTreeMap<String, LinkedSource>,
}
```

Similarly for `AnnotationsV2` with `#specification`, `#section`, `#requirement`, `#impl`. The `ImplAnnotation.anno_type` field needs `#[serde(rename = "type")]` to match the JSON output (following the existing pattern in the current `AnnotationV2`).

### Shared types

```rust
/// A reference to a single contiguous byte range within an inline source
/// file. Used for specification and section annotations, which always
/// cover one contiguous span.
pub struct SourceRef {
    pub src: String,       // "src-xxxx" key into sources.inline
    pub start: usize,      // start byte offset (inclusive)
    pub end: usize,        // end byte offset (exclusive)
}

/// A reference to one or more (possibly disjoint) byte ranges within an
/// inline source file. Used wherever a matched quote can span multiple
/// non-contiguous regions of the spec (e.g., IETF RFC page breaks).
/// `ranges` is canonically sorted ascending and deduplicated.
pub struct SourceRanges {
    pub src: String,               // "src-xxxx" key into sources.inline
    pub ranges: Vec<ByteRange>,    // ≥1 disjoint ranges
}

/// A reference to a line in a source code file.
pub struct SourceLocation {
    pub src: String,           // "lnk-xxxx" key into sources.linked
    pub line: Option<usize>,   // line number in the file
}

pub struct ByteRange {
    pub start: usize,  // start byte offset (inclusive)
    pub end: usize,    // end byte offset (exclusive)
}
```

### Specification annotation

One per specification document. Title and format describe the specification content, so they belong in an annotation referencing that content — not on the source itself (which is just raw storage).

```rust
pub struct SpecificationAnnotation {
    pub source: SourceRef,         // full byte range (start: 0, end: file length)
    pub title: Option<String>,     // e.g., "RFC 9000"
    pub format: String,            // "ietf" or "markdown"
}
```

### Section annotation

One per section in each specification.

```rust
pub struct SectionAnnotation {
    pub source: SourceRef,         // byte range of the section
    pub short_name: String,        // e.g., "section-2.1"
    pub long_name: Option<String>, // e.g., "Request Methods"
}
```

### Requirement annotation

A requirement extracted from a specification or authored in a requirement TOML file. **No `section` field** — the section is inferrable from byte range containment within section annotations.

```rust
pub struct RequirementAnnotation {
    /// Authoring site: where this requirement was declared. Points at a
    /// `.duvet/requirements/**/*.toml` file for extracted/hand-authored
    /// requirements, or at the source file containing an inline
    /// `//= type=spec` comment.
    pub source: SourceLocation,
    /// Spec byte range(s) this requirement represents. May contain
    /// multiple disjoint ranges when the matched quote spans regions the
    /// spec parser normalized away (e.g., IETF RFC page breaks) — see
    /// "Disjoint coverage ranges" below.
    pub origin: SourceRanges,
    pub level: AnnotationLevel,    // AUTO, MAY, SHOULD, MUST

    /// Coverage map: impl annotation "cite-xxxx" ID → list of byte ranges
    /// in the spec file covered by that impl annotation's quote match.
    /// An impl annotation may appear in multiple requirement annotations'
    /// coverage maps (many-to-many relationship).
    ///
    /// The value is a Vec for the same reason `origin.ranges` is — a
    /// single quote match can span multiple disjoint byte ranges.
    pub coverage: BTreeMap<String, Vec<ByteRange>>,
}
```

One logical authoring site produces exactly one `RequirementAnnotation`, regardless of how many disjoint byte ranges the quote matched. The `req-` ID hash includes the full sorted range list so that fragments of the same logical requirement collapse into a single entry.

### Impl annotation

A developer-authored annotation in source code. **No `section` field** — inferrable from `target.ranges` byte range containment. **No `quote` field** — the matched spec text is recoverable from `sources[target.src].contents[target.ranges[i].start..target.ranges[i].end]`. **`comment` is kept** — it's a separate field (exception reasons, todo reasons), emitted in v1 JSON and displayed by the frontend.

```rust
pub struct ImplAnnotation {
    pub source: SourceLocation,
    /// The specification file this annotation targets and the matched
    /// byte range(s) within it. `target.ranges` may contain multiple
    /// disjoint ranges when the quote spans regions the spec parser
    /// normalized away — see "Disjoint coverage ranges" below.
    pub target: SourceRanges,
    pub anno_type: AnnotationType,     // CITATION, TEST, IMPLICATION, EXCEPTION, TODO
    pub level: AnnotationLevel,
    pub comment: Option<String>,
    pub feature: Option<String>,
    pub tracking_issue: Option<String>,
    pub tags: Vec<String>,
}
```

An annotation targets exactly one specification file (the annotation format `//= <url>#<section>` references a single spec). The disjoint ranges are non-contiguous byte ranges within that one file (e.g., across IETF RFC page breaks). Nesting `src` and `ranges` inside `target: SourceRanges` keeps the coupling between them structurally explicit — `ranges` are meaningless without the `src` they index into.

### Disjoint coverage ranges

`ImplAnnotation.target.ranges`, `RequirementAnnotation.origin.ranges`, and `RequirementAnnotation.coverage` values are all arrays because a single quote match can produce multiple disjoint byte ranges in the original specification file.

**How this happens:** The `View` struct (`text/view.rs`) concatenates non-contiguous spec lines into a single searchable string. IETF RFCs have page breaks (form feeds + headers/footers) that interrupt section text. The IETF parser strips these, but the underlying `Slice` objects still reference the original file offsets. When `Annotation::quote_range()` finds a match in the concatenated view, `View::ranges()` maps it back to the original file coordinates — yielding multiple disjoint `Slice` items if the match spans a page break.

**Concrete example:**

```
Spec file (rfc9000.txt):
  offset 1000: "A server MUST accept"     ← section line
  offset 1050: "(page break / header)"    ← stripped by parser
  offset 1100: "both BREW and POST"       ← section line (continued)
```

The View concatenates to: `"A server MUST accept both BREW and POST"`. A developer writes:

```rust
//= https://rfc-editor.org/rfc/rfc9000#section-2.1
//# A server MUST accept both BREW and POST
```

The quote matches the full concatenated string. `View::ranges()` yields two `Slice` items: `1000..1020` and `1100..1118`. In `reference.rs`, each becomes a separate `Reference` struct pointing to the same `AnnotationWithId`:

```rust
for text in contents.ranges(range) {
    references.push(Reference { target, text, annotation: annotation.clone() });
}
```

**Why a single range doesn't work:** Using the bounding range `1000..1118` would incorrectly claim coverage of the page break text at offsets 1050-1099. Using only the first range `1000..1020` would lose coverage of `"both BREW and POST"`. The array representation preserves exact coverage.

**Impact on merge:** Merging coverage = union of maps. For each impl annotation key, concatenate range lists from both packages, then sort and deduplicate. Stats are computed by iterating all ranges per impl annotation.

### Computing stats from byte ranges

Given a requirement annotation with `source` (byte range) and `coverage` map, plus the type of each related annotation (looked up from the annotations map):

- `spec` = `source.end - source.start` (total bytes in the requirement)
- `citation` = count of unique byte offsets covered by CITATION annotations (union of all their ranges)
- `test` = count of unique byte offsets covered by TEST annotations
- `implication`, `exception`, `todo` = same pattern
- `incomplete` = `spec` - count of byte offsets covered by any annotation (matching `status.rs` logic: offsets covered by exception, implication, or citation∪test are removed from the incomplete set)
- `related` = keys of the coverage map

The current `status.rs` logic for computing `incomplete`:
1. Start with all spec byte offsets
2. Remove offsets covered by exceptions
3. Remove offsets covered by implications
4. Remove offsets in citation ∪ test (an offset with *either* citation or test is considered addressed)
5. `incomplete` = remaining count

This is fully reproducible from byte ranges + annotation types.

### Reconstructing the specification view

For v2→v1 conversion or an updated frontend:

1. **Spec list**: Group specification annotations by inline source ID. Title from `SpecificationAnnotation.title`, format from `SpecificationAnnotation.format`.
2. **Section list**: Group section annotations by inline source ID. Title from `SectionAnnotation.long_name`, ID from `SectionAnnotation.short_name`.
3. **Lines**: Read raw text from inline source `contents`. Split into lines. For each line, find overlapping requirement and impl annotation byte ranges to compute segmentation.
4. **Requirements**: All requirement annotations whose `source` byte range falls within a section annotation's byte range are that section's requirements.

### Source path handling

`Annotation.source` is a `duvet_core::path::Path` that stores whatever path the `glob` crate returned when scanning source files. Since config patterns are relative (e.g., `"src/**/*.rs"`), `glob` returns relative paths (e.g., `src/lib.rs`). These relative paths are already suitable for `LinkedSource.file_name` — no stripping of project root is needed.

The v1 JSON already uses `annotation.source.to_string_lossy()` to emit relative paths like `"package-a/src/impl.rs"`. The v2 schema uses the same value for `LinkedSource.file_name`.

**Note:** `duvet_core::path::Path` has a `Display` impl that strips the current directory prefix, but `to_string_lossy()` bypasses that — it goes through `std::path::Path::to_string_lossy()` directly, returning the raw path as stored.

**Risk:** If duvet is run from a subdirectory or with absolute glob patterns, paths could be absolute. The `source.root` config field exists but is currently unused (`let _ = &source.root;` in `project.rs`). This is a pre-existing issue outside the scope of this plan.

### Example v2 output

```json
{
  "version": "2.0",
  "issue_links": [
    "https://github.com/org/repo/issues",
    "https://issues.example.com/project"
  ],
  "repositories": {
    "repo-d6e7f8a9b0c12345": {
      "blob_link": "https://github.com/org/repo/blob/main"
    }
  },
  "sources": {
    "https://awslabs.github.io/duvet/v2/sources.json#inline": {
      "src-a3f7b2c1e9d04856": {
        "file_name": "rfc9000.txt",
        "contents": "... full RFC text ..."
      }
    },
    "https://awslabs.github.io/duvet/v2/sources.json#linked": {
      "lnk-b4c8d3e2f1a05967": {
        "file_name": "src/server.rs",
        "repository": "repo-d6e7f8a9b0c12345"
      },
      "lnk-c5d9e4f3a2b16a78": {
        "file_name": "tests/server_test.rs",
        "repository": "repo-d6e7f8a9b0c12345"
      },
      "lnk-d7e8f9a0b1c23456": {
        "file_name": ".duvet/requirements/rfc9000/section-2.1.toml",
        "repository": "repo-d6e7f8a9b0c12345"
      }
    }
  },
  "annotations": {
    "https://awslabs.github.io/duvet/v2/annotations.json#specification": {
      "spc-a1b2c3d4e5f60001": {
        "source": { "src": "src-a3f7b2c1e9d04856", "start": 0, "end": 98765 },
        "title": "RFC 9000",
        "format": "ietf"
      }
    },
    "https://awslabs.github.io/duvet/v2/annotations.json#section": {
      "spc-e5f6a7b8c9d01234": {
        "source": { "src": "src-a3f7b2c1e9d04856", "start": 12000, "end": 13000 },
        "short_name": "section-2.1",
        "long_name": "Request Methods"
      }
    },
    "https://awslabs.github.io/duvet/v2/annotations.json#requirement": {
      "req-f7a3b2c1e9d04856": {
        "source": { "src": "lnk-d7e8f9a0b1c23456", "line": 10 },
        "origin": {
          "src": "src-a3f7b2c1e9d04856",
          "ranges": [
            { "start": 12340, "end": 12405 }
          ]
        },
        "level": "MUST",
        "coverage": {
          "cite-c3d4e5f6a7b80912": [
            { "start": 12340, "end": 12360 },
            { "start": 12400, "end": 12405 }
          ],
          "cite-d4e5f6a7b8091234": [
            { "start": 12350, "end": 12405 }
          ]
        }
      }
    },
    "https://awslabs.github.io/duvet/v2/annotations.json#impl": {
      "cite-c3d4e5f6a7b80912": {
        "source": { "src": "lnk-b4c8d3e2f1a05967", "line": 42 },
        "target": {
          "src": "src-a3f7b2c1e9d04856",
          "ranges": [
            { "start": 12340, "end": 12360 },
            { "start": 12400, "end": 12405 }
          ]
        },
        "type": "CITATION",
        "level": "AUTO",
        "comment": "Implemented in handle_request()"
      },
      "cite-d4e5f6a7b8091234": {
        "source": { "src": "lnk-c5d9e4f3a2b16a78", "line": 15 },
        "target": {
          "src": "src-a3f7b2c1e9d04856",
          "ranges": [
            { "start": 12350, "end": 12405 }
          ]
        },
        "type": "TEST",
        "level": "AUTO"
      }
    }
  }
}
```

**Page-break example** — an impl annotation whose quote spans an IETF RFC page break:

```json
{
  "cite-e5f6a7b8c9d01234": {
    "source": { "src": "lnk-b4c8d3e2f1a05967", "line": 87 },
    "target": {
      "src": "src-a3f7b2c1e9d04856",
      "ranges": [
        { "start": 1000, "end": 1020 },
        { "start": 1100, "end": 1118 }
      ]
    },
    "type": "CITATION",
    "level": "AUTO"
  }
}
```

The quote `"A server MUST accept both BREW and POST"` matched across a page break. Offsets 1020-1099 contain the page break header/footer and are not covered. The two disjoint ranges precisely capture the actual spec text matched.

### Implementation checklist

#### Building the v2 report from `ReportResult`

The conversion from `ReportResult` to the new `ReportV2` requires correlating annotations with their matched references to populate `target.ranges` and `coverage`. The current `from_report_result()` iterates annotations and targets separately; the new version must cross-reference them. The conversion proceeds in four steps:

**Step 1: Build entity ID infrastructure.**

- Compute repository IDs from unique blob_links. Each unique blob_link (from per-source config or global `report.html.blob-link`) produces a `repo-` prefixed ID via `FNV-1a(blob_link)`.
- Compute inline source IDs by reading spec file contents from `TargetReport.specification`. The backing `SourceFile` is accessible from any section's line `Slice`: `section.full_title.file()` returns the `SourceFile`, which derefs to `&str` for the full contents. Hash the contents to produce `src-` prefixed IDs.
- Compute linked source IDs from annotation source paths + repository IDs. `annotation.source.to_string_lossy()` gives the relative file path (see "Source path handling" below). Hash `file_name \0 repository_id` (or `file_name \0 ""` when no repository) to produce `lnk-` prefixed IDs.

**Step 2: Build impl annotations with `target.ranges`.**

- Iterate all `TargetReport.references` across all targets.
- Group non-SPEC references by `cite-` ID.
- For each group, collect `(Reference.start(), Reference.end())` byte ranges into `ImplAnnotation.target.ranges`. Sort and dedup.
- A single annotation may produce multiple `Reference`s (from `View::ranges()` when a quote spans a page break), each contributing a separate `ByteRange` to the array.

**Step 3: Build requirement annotations with `coverage`.**

Two-pass, per target:

1. **Group SPEC references by authoring site.** Key: `(origin src-, source lnk-, anno_line)`. Collect each reference's `(start, end)` into the group's range list. This guarantees that one logical authoring site (one TOML `[[spec]]` or one inline `//= type=spec` comment) produces exactly one `RequirementAnnotation` even when its quote matched N disjoint byte ranges.
2. **Emit one `RequirementAnnotation` per group.** Sort and dedup the group's ranges. Compute `req_id(origin_id, &ranges, source_id, anno_line)`. Populate `origin: SourceRanges { src: origin_id, ranges }`.
3. **Build `coverage` by iterating each origin range.** For every non-SPEC reference on the target, clamp `(ref.start, ref.end)` to each `(origin_range.start, origin_range.end)` and insert a `ByteRange` under that reference's `cite-` ID if the clamp is non-empty. A single impl reference can contribute under multiple origin ranges of the same requirement; dedup each coverage list after population.

This matches the existing `StatusMap::populate()` semantics, which uses `coverage.range(r.start()..r.end())` to iterate only the offsets *within* the SPEC reference's byte range. The new logic preserves `(start, end)` ranges attributed to specific impl annotations instead of just counting offsets, and it no longer fragments one logical requirement into N entries.

**Step 4: Build specification and section annotations.**

- Specification annotation: `source` = `SourceRef { src: src_id, start: 0, end: file_length }`. File length from `source_file.len()`. Title from `Specification.title`, format from `Specification.format`.
- Section annotation: `source` = `SourceRef { src: src_id, start: full_title.range().start, end: last_line_end }`. Compute `last_line_end` as the maximum `line.range().end` across all `Line::Str` lines in the section. `short_name` from `Section.id`, `long_name` from `Section.title`.

Changes to `json_v2.rs`:

1. Add `InlineSource`, `LinkedSource`, `SourcesV2` (JSON keys `#inline`/`#linked`), `Repository`, `repositories` map to `ReportV2`
2. Add `AnnotationsV2` with `specification`, `section`, `requirement`, `impl` maps (JSON keys are schema URLs, map keys are `spc-`/`req-`/`cite-` prefixed IDs)
3. Add `SourceRef`, `SourceRanges`, `SourceLocation`, `SpecificationAnnotation`, `SectionAnnotation`, `RequirementAnnotation`, `ImplAnnotation`, `ByteRange` structs (no `id` field — ID is the map key). `SourceRef` is used where the referenced region is always contiguous (specification and section annotations); `SourceRanges` is used where it may be disjoint (requirement origin, impl target).
4. Remove `SpecificationV2`, `SectionV2`, `LineV2`, `LineSegmentV2`, `AnnotationV2`, `CoverageStatus` structs
5. Remove `specifications` and `coverage` fields from `ReportV2`
6. Rename `issue_link` → `issue_links: Vec<String>`, remove `blob_link` from `ReportV2`. For initial single-package report generation, `issue_links` is populated by wrapping the single `ReportResult.issue_link` value in a one-element `Vec` (or empty `Vec` if `None`). During merge (Phase 3), `issue_links` from all input reports are concatenated and deduplicated.
7. Relocate `segment_line()` and bitmap status functions (`encode_annotation`, `merge_status`, bitmask constants) to a new v2→v1 conversion module (Phase 4.2). Remove `build_specification_v2()`.
8. Update `ReportV2::from_report_result()`:
   - Build repositories map from unique blob links (hash blob_link → `repo-` prefix)
   - Build inline sources from parsed specifications (hash contents → `src-` prefix)
   - Build linked sources from source code files (hash path + repo ID → `lnk-` prefix)
   - Build specification annotations from parsed spec metadata (title, format, full byte range)
   - Build section annotations from parsed sections (short_name, long_name, section byte range)
   - Build requirement annotations with source references and coverage byte range lists
   - Build impl annotations with source locations and target source range lists
9. Update `read_report_v2()` / `write_report_v2()` for new schema
10. Update all tests (roundtrip, property tests)
11. Keep version at `"2.0"` (schema was never released, so no compatibility concern)

**Note:** After Phase 2.5 is implemented, `.kiro/docs/REHYDRATE_REPORT_RESULT.md` must be updated. It describes rehydration from the initial v2 schema (with `SpecificationV2`, `SectionV2`, `LineV2`, etc.) which will be replaced. The new schema stores raw text + byte ranges instead of pre-segmented lines, which simplifies rehydration (no need to reconstruct segment boundaries) but changes the approach entirely.

---

## Phase 3: Merge Functionality

### 3.1 Implement `duvet merge` command

```
duvet merge \
  --input package-a/.duvet/report-v2.json \
  --input package-b/.duvet/report-v2.json \
  --output merged-report.json \
  --html merged-report.html \
  --snapshot merged-snapshot.txt
```

### 3.2 Merge logic

- **Repositories**: Union by ID (hash of blob link). Different packages with the same blob link deduplicate naturally.
- **Inline sources**: Union by hash (identical specs deduplicate naturally)
- **Linked sources**: Union by ID (hash of file path + repository ID)
- **Specification annotations**: Union by stable ID. Same spec from different packages produces identical annotations.
- **Section annotations**: Union by stable ID. Same section from different packages produces identical annotations.
- **Requirement annotations**: Union by stable ID. Coverage maps merged by union of entries. For each impl annotation key present in both packages, concatenate range lists, sort, and deduplicate. If both packages have identical ranges for the same key, dedup collapses them.
- **Impl annotations**: Union by stable ID with package attribution
- **Coverage recomputation**: After merge, coverage stats are recomputed from the merged byte-range maps. Segmentation is recomputed from inline sources + merged annotations.
- **issue_links**: Concatenate and deduplicate across input reports.

### 3.3 Conflict handling

When two input reports contain the same entity ID, the merge must decide whether the entries are compatible. The rules are per-entity-type:

| Entity | Conflict possible? | Resolution |
|--------|-------------------|------------|
| `repo-` | No — ID is hash of `blob_link`, so same ID = identical content | Union by ID |
| `src-` | Only `file_name` mismatch (same contents, different name) | Error. Users must ensure consistent file naming across packages. |
| `lnk-` | No — ID is hash of `file_name` + `repository_id` | Union by ID |
| `spc-` specification | `title` or `format` mismatch | Error (spec version drift between packages) |
| `spc-` section | `short_name` or `long_name` mismatch | Error (spec version drift between packages) |
| `req-` requirement | `level` mismatch; `coverage` differs | Error on `level` mismatch. Union `coverage` maps (additive — this is the core merge operation). |
| `cite-` | Core fields or metadata differ | Error if `anno_type`, `level`, or `target` differ (indicates scanning inconsistency — same source file produced different results). Warn if only `comment`, `feature`, `tracking_issue`, or `tags` differ (metadata drift); take the value from the first input report. |

**Why `cite-` conflicts indicate bugs:** A `cite-` ID collision means the same source file, same line, same target spec was scanned by two packages and produced different core results. This should not happen in normal operation. The most likely cause is different duvet versions or different spec file versions across packages.

### 3.4 Overlapping source patterns

In monorepo configurations, multiple packages may scan overlapping source file sets. For example, package A scans `src/**/*.rs` while package B scans `src/server/**/*.rs` — both would process `src/server/handler.rs`.

This is handled correctly by the ID scheme: annotations from the same source file, same line, same target spec, and same repository produce identical `cite-` IDs. Merge deduplicates them naturally via union-by-ID. The merged report shows each annotation once regardless of how many packages scanned it.

**Caveat:** If packages use different `blob-link` values for the same source file, the `lnk-` IDs will differ (since `lnk-` includes repository_id), producing duplicate `cite-` annotations with different IDs pointing to the same logical annotation. Users should ensure consistent `blob-link` configuration for shared source files across packages.

---

## Phase 4: Frontend Updates

### 4.1 Update React frontend ✅ COMPLETED (commit 75acb7c4)

- Frontend now uses annotation's `blob_link` if present, falls back to global
- Implemented in `www/src/result.js` via `createBlobLinker()` function

### 4.2 Generate merged HTML (v2→v1 conversion)

Implement a `v2_to_v1` conversion module that transforms a v2 report into the v1 JSON format consumed by the existing React frontend. This enables HTML report generation from merged v2 data without frontend changes.

**Relocated code:** `segment_line()`, `encode_annotation()`, `merge_status()`, and the bitmask constants from the current `json_v2.rs` move to this module. They are needed for reconstructing v1's segmented lines and `refs` table. `build_specification_v2()` is removed (no longer needed).

**The v2→v1 conversion reconstructs:**
1. `specifications` from specification annotations (title, format) and section annotations (short_name, long_name), with lines from inline sources, segmented using requirement and impl annotation byte ranges via `segment_line()`
2. `annotations` array with integer IDs (positional)
3. `statuses` map with byte counts computed from requirement annotation coverage byte range lists
4. `refs` table (full 256-entry enumeration of status flag combinations)
5. `issue_link` from first entry in `issue_links` (v1 only supports one)
6. `blob_link` resolved from linked source → repository → `blob_link`
7. Per-annotation `blob_link` resolved the same way

**Fallback:** If v2→v1 conversion proves infeasible or too complex, the alternative is updating the React frontend to consume v2 directly, which eliminates the v1 dependency entirely.

---

## Implementation Order

1. ~~**Phase 1.1 + 4.1**: Per-source blob-link support~~ ✅ **COMPLETED** (commit 75acb7c4)
2. ~~**Phase 1.2**: Cite-style stable annotation IDs~~ ✅ **COMPLETED**
3. ~~**Phase 2.1-2.3**: Initial v2 JSON format~~ ✅ **COMPLETED**
4. ~~**Phase 1.3**: Entity-typed deterministic ID functions (can be tested independently)~~ ✅ **COMPLETED**
5. ~~**Phase 2.5**: New v2 schema (depends on 1.3)~~ ✅ **COMPLETED**
6. **Phase 3.1-3.2**: Merge command
7. **Phase 4.2**: v2→v1 conversion layer or frontend update

### Phase 2.5 transition notes

Phase 2.5 rewrites `json_v2.rs` but does not require resetting to main. The following artifacts from completed phases survive and should be preserved:

**Keep as-is:**
- Phase 1.1 blob-link: config schema, `Annotation.blob_link` field, comment parser propagation, frontend `createBlobLinker()` — all foundational
- `ids.rs` module — contains `fnv1a_64()` (moved from `annotation.rs`) and all 5 entity-typed ID functions (`repo_id`, `src_id`, `lnk_id`, `spc_id`, `cite_id`) with property tests
- CLI plumbing: `--json-v2` flag, `DUVET_INTERNAL_CI_JSON_V2` env var, `report.rs` wiring
- Integration test infrastructure in `xtask/tests.rs` — v2 snapshot generation stays, snapshots regenerate

**Completed during Phase 2.5:**
- Removed `stable_annotation_id()` from `annotation.rs` and its tests
- Removed `AnnotationWithId.stable_id` field entirely — `cite_id()` is computed in `from_report_result()` where all data (spec contents, repo IDs) is available
- Replaced all v2 serde structs with entity-typed schema
- Rewrote `from_report_result()` with 6-step entity ID infrastructure
- Updated `read_report_v2()` / `write_report_v2()` for new schema
- Regenerated all 18 v2 integration test snapshots

**Deleted (must be reimplemented in Phase 4.2 for v2→v1 conversion):**
- `segment_line()` and `SegmentRef` — line segmentation from annotation byte ranges
- `encode_annotation()`, `merge_status()`, `encode_status()`, `decode_status()`, `format_status()` — bitmask encoding/decoding functions
- `BIT_SPEC`, `BIT_CITATION`, `BIT_IMPLICATION`, `BIT_TEST`, `BIT_EXCEPTION`, `BIT_TODO`, `LEVEL_MASK`, `LEVEL_SHIFT` — bitmask constants
- `AnnotationLevel::to_bits()` / `from_bits()` — level encoding helpers
- `build_specification_v2()` — old spec builder (no longer needed; v2→v1 will need a different approach)
- All old v2 structs: `SpecificationV2`, `SectionV2`, `LineV2`, `LineSegmentV2`, `AnnotationV2`, `CoverageStatus`
- All old v2 tests: bitmask roundtrip, segmentation completeness/accuracy, merge correctness/associativity/commutativity, format_status completeness

---

## Additional Considerations

### Merge config format

Since per-source `blob-link` is now stored in annotations in the JSON report, the merge config no longer needs to specify blob-link per input:

```toml
[[input]]
path = "package-a/.duvet/report-v2.json"

[[input]]
path = "package-b/.duvet/report-v2.json"
```

Each package's `.duvet/config.toml` should specify `blob-link` in its `[[source]]` or `[[repository]]` blocks, and these will be preserved in the merged report via the `repositories` map.

### Partial coverage tracking

Track per-package contribution: "package A covers 60% of section X, package B covers 40%". Current byte-offset model supports this; v2 format should preserve detail.

### Incremental merging

With many packages, merge incrementally (A+B → AB, AB+C → ABC). Stable IDs make this work.

### Spec version drift

If packages reference different spec versions, need strategy. Simplest: require identical specs, fail on mismatch.

---

## Appendix: Complete v2 JSON Schema (after Phase 2.5)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Duvet Report v2",
  "type": "object",
  "required": ["version", "sources", "annotations"],
  "properties": {
    "version": {
      "type": "string",
      "const": "2.0"
    },
    "issue_links": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Issue tracker URL prefixes"
    },
    "repositories": {
      "type": "object",
      "description": "Map of 'repo-' prefixed ID to repository metadata",
      "additionalProperties": { "$ref": "#/$defs/Repository" }
    },
    "sources": {
      "type": "object",
      "description": "Sources grouped by type, keyed by schema URLs",
      "properties": {
        "https://awslabs.github.io/duvet/v2/sources.json#inline": {
          "type": "object",
          "description": "Map of 'src-' prefixed ID to inline source (specification file with full contents)",
          "additionalProperties": { "$ref": "#/$defs/InlineSource" }
        },
        "https://awslabs.github.io/duvet/v2/sources.json#linked": {
          "type": "object",
          "description": "Map of 'lnk-' prefixed ID to linked source (source code file metadata)",
          "additionalProperties": { "$ref": "#/$defs/LinkedSource" }
        }
      }
    },
    "annotations": {
      "type": "object",
      "description": "Annotations grouped by type, keyed by schema URLs",
      "properties": {
        "https://awslabs.github.io/duvet/v2/annotations.json#specification": {
          "type": "object",
          "description": "Map of 'spc-' prefixed ID to specification annotation (document metadata)",
          "additionalProperties": { "$ref": "#/$defs/SpecificationAnnotation" }
        },
        "https://awslabs.github.io/duvet/v2/annotations.json#section": {
          "type": "object",
          "description": "Map of 'spc-' prefixed ID to section annotation (section metadata)",
          "additionalProperties": { "$ref": "#/$defs/SectionAnnotation" }
        },
        "https://awslabs.github.io/duvet/v2/annotations.json#requirement": {
          "type": "object",
          "description": "Map of 'req-' prefixed ID to requirement annotation",
          "additionalProperties": { "$ref": "#/$defs/RequirementAnnotation" }
        },
        "https://awslabs.github.io/duvet/v2/annotations.json#impl": {
          "type": "object",
          "description": "Map of 'cite-' prefixed ID to impl annotation",
          "additionalProperties": { "$ref": "#/$defs/ImplAnnotation" }
        }
      }
    }
  },
  "$defs": {
    "Repository": {
      "type": "object",
      "description": "A source code repository. Keyed by 'repo-' prefixed ID (hash of blob_link).",
      "required": ["blob_link"],
      "properties": {
        "blob_link": {
          "type": "string",
          "description": "URL prefix for source file links (e.g., 'https://github.com/org/repo/blob/main')"
        }
      }
    },
    "InlineSource": {
      "type": "object",
      "description": "A specification file with full contents. Keyed by 'src-' prefixed ID (hash of contents). Metadata (title, format, sections) is captured by specification and section annotations.",
      "required": ["file_name", "contents"],
      "properties": {
        "file_name": {
          "type": "string",
          "description": "Original file name (e.g., 'rfc9000.txt', 'spec.md')"
        },
        "contents": {
          "type": "string",
          "description": "Full text content of the specification file"
        }
      }
    },
    "LinkedSource": {
      "type": "object",
      "description": "A source code file (metadata only, contents may be added in the future). Keyed by 'lnk-' prefixed ID (hash of file_name + repository ID).",
      "required": ["file_name"],
      "properties": {
        "file_name": {
          "type": "string",
          "description": "File path (e.g., 'src/server.rs')"
        },
        "repository": {
          "type": "string",
          "description": "'repo-' prefixed key into the repositories map"
        }
      }
    },
    "SpecificationAnnotation": {
      "type": "object",
      "description": "Metadata about an entire specification document. Keyed by 'spc-' prefixed ID.",
      "required": ["source", "format"],
      "properties": {
        "source": {
          "$ref": "#/$defs/SourceRef",
          "description": "Full byte range of the specification file (start: 0, end: file length)"
        },
        "title": {
          "type": "string",
          "description": "Document title (e.g., 'RFC 9000')"
        },
        "format": {
          "type": "string",
          "description": "Document format, determines rendering behavior",
          "enum": ["ietf", "markdown"]
        }
      }
    },
    "SectionAnnotation": {
      "type": "object",
      "description": "Metadata about a section within a specification. Keyed by 'spc-' prefixed ID.",
      "required": ["source", "short_name"],
      "properties": {
        "source": {
          "$ref": "#/$defs/SourceRef",
          "description": "Byte range of the section within the specification file"
        },
        "short_name": {
          "type": "string",
          "description": "Section identifier (e.g., 'section-2.1', 'appendix-A')"
        },
        "long_name": {
          "type": "string",
          "description": "Human-readable section title (e.g., 'Request Methods')"
        }
      }
    },
    "RequirementAnnotation": {
      "type": "object",
      "description": "A requirement extracted from a specification document. Keyed by 'req-' prefixed ID. Section is inferrable from byte range containment within section annotations.",
      "required": ["source", "origin", "level"],
      "properties": {
        "source": {
          "$ref": "#/$defs/SourceLocation",
          "description": "Authoring site: where this requirement was declared (TOML file or inline //= type=spec comment)"
        },
        "origin": {
          "$ref": "#/$defs/SourceRanges",
          "description": "Spec byte range(s) this requirement represents. May contain multiple disjoint ranges when the matched quote spans regions the spec parser normalized away (e.g., IETF RFC page breaks)."
        },
        "level": {
          "type": "string",
          "enum": ["AUTO", "MAY", "SHOULD", "MUST"]
        },
        "coverage": {
          "type": "object",
          "description": "Map of related impl annotation 'cite-' ID to list of byte ranges covered. Arrays because a single quote match can span disjoint byte ranges (e.g., across IETF RFC page breaks).",
          "additionalProperties": {
            "type": "array",
            "items": { "$ref": "#/$defs/ByteRange" }
          }
        }
      }
    },
    "ImplAnnotation": {
      "type": "object",
      "description": "A developer-authored annotation in source code (citation, test, etc.). Keyed by 'cite-' prefixed ID. Section is inferrable from target.ranges byte range containment within section annotations.",
      "required": ["source", "target", "type"],
      "properties": {
        "source": {
          "$ref": "#/$defs/SourceLocation",
          "description": "Location in the source code file (references a linked source)"
        },
        "target": {
          "$ref": "#/$defs/SourceRanges",
          "description": "The specification file this annotation targets and the matched byte range(s) within it. May contain multiple disjoint ranges when the quote spans regions the spec parser normalized away (e.g., IETF RFC page breaks)."
        },
        "type": {
          "type": "string",
          "enum": ["CITATION", "TEST", "IMPLICATION", "EXCEPTION", "TODO"]
        },
        "level": {
          "type": "string",
          "enum": ["AUTO", "MAY", "SHOULD", "MUST"],
          "default": "AUTO"
        },
        "comment": { "type": "string" },
        "feature": { "type": "string" },
        "tracking_issue": { "type": "string" },
        "tags": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "SourceRef": {
      "type": "object",
      "description": "A reference to a single contiguous byte range within an inline source file",
      "required": ["src", "start", "end"],
      "properties": {
        "src": {
          "type": "string",
          "description": "'src-' prefixed key into the sources map (inline type)"
        },
        "start": {
          "type": "integer",
          "description": "Start byte offset (inclusive, absolute in the source file)"
        },
        "end": {
          "type": "integer",
          "description": "End byte offset (exclusive, absolute in the source file)"
        }
      }
    },
    "SourceRanges": {
      "type": "object",
      "description": "A reference to one or more (possibly disjoint) byte ranges within an inline source file. Ranges are canonically sorted ascending.",
      "required": ["src", "ranges"],
      "properties": {
        "src": {
          "type": "string",
          "description": "'src-' prefixed key into the sources map (inline type)"
        },
        "ranges": {
          "type": "array",
          "items": { "$ref": "#/$defs/ByteRange" }
        }
      }
    },
    "SourceLocation": {
      "type": "object",
      "description": "A reference to a line in a source code file",
      "required": ["src"],
      "properties": {
        "src": {
          "type": "string",
          "description": "'lnk-' prefixed key into the sources map (linked type)"
        },
        "line": {
          "type": "integer",
          "description": "Line number in the source file"
        }
      }
    },
    "ByteRange": {
      "type": "object",
      "required": ["start", "end"],
      "properties": {
        "start": {
          "type": "integer",
          "description": "Start byte offset (absolute, in the source file)"
        },
        "end": {
          "type": "integer",
          "description": "End byte offset (exclusive, absolute, in the source file)"
        }
      }
    }
  }
}
```
