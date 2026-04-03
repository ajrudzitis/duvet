# Duvet Report Engine: Deep Dive

This document explains how Duvet generates compliance reports, with a focus on the matching process between specification text and source code annotations.

## High-Level Pipeline

The report engine (`Report::exec` in `duvet/src/report.rs`) runs a sequential pipeline:

```
1. Load config & extract specification requirements
2. Scan source files for annotation comments
3. Parse annotations from those sources
4. Load specification documents (IETF RFCs, Markdown)
5. Build the annotation reference map (group annotations by target+section)
6. Match annotation quotes against specification section text → produce References
7. Sort references into per-target reports and compute coverage status
8. Write output reports (HTML, JSON, snapshot, LCOV)
```

## Step-by-Step Breakdown

### 1. Source Scanning & Annotation Parsing

`annotation::query()` (`annotation.rs:45`) spawns async tasks per source file. Each source file is tokenized by the comment tokenizer (`comment/tokenizer.rs`) which looks for the configured meta/content prefixes (default `//=` and `//#`), then parsed by the comment parser (`comment/parser.rs`) into `Annotation` structs.

An `Annotation` contains:
- `target` — the spec URL or path, including the `#section-id` fragment
- `quote` — the quoted requirement text from the annotation's content lines
- `anno` — the annotation type (`Citation`, `Test`, `Exception`, `Implication`, `Todo`, `Spec`)
- `level` — requirement level (`Auto`, `Must`, `Should`, `May`)
- `source` — the file path where the annotation was found
- `anno_line` — the line number in the source file

### 2. Specification Loading

`annotation::specifications()` (`annotation.rs:30`) collects all unique `Target`s from annotations, then `target::query()` downloads/loads each specification document and parses it.

Parsing is format-dependent (`specification.rs`, `Format::parse`):
- **IETF**: Tokenized via `specification/ietf/tokenizer.rs` (handles page breaks, line breaks, section headers) then parsed into `Section`s by `specification/ietf/parser.rs`.
- **Markdown**: Tokenized via `specification/markdown/tokenizer.rs` (uses `pulldown-cmark`) then parsed into `Section`s.
- **Auto**: Heuristic — if the file extension is `.md`/`.markdown` or content starts with `#` or `[//]:`, it's Markdown; otherwise IETF.

A `Specification` is a map of `section_id → Section`. Each `Section` has an `id`, `title`, `full_title` (a `Slice` pointing into the original file), and `lines` (a `Vec<Line>` of content slices).

### 3. Building the Reference Map

`annotation::reference_map()` (`annotation.rs:75`) groups all annotations by their `(Target, Option<section_id>)` key. The target is derived by splitting the annotation's target string on `#`. This produces an `AnnotationReferenceMap`:

```
HashMap<(Arc<Target>, Option<Arc<str>>), Arc<[AnnotationWithId]>>
```

Each entry says: "these annotations all point at this section of this specification."

### 4. The Matching Process (Core Engine)

This is the heart of the report. `reference::query()` (`reference.rs:38`) iterates over every entry in the reference map and spawns `build_references()` for each `(target, section, annotations)` tuple.

#### `build_references()` (`reference.rs:77`)

For each group of annotations targeting the same spec section:

1. **Section lookup**: Find the `Section` in the `Specification` by its `section_id`. For IETF specs, this includes fallback logic that strips `section-`/`appendix-` prefixes and retries. If the section is missing, an error is emitted for every annotation in the group.

2. **Build a View**: `section.view()` concatenates all non-break lines of the section into a single normalized string (`text::view::View`). The `View` maintains a `byte_map` that maps each byte offset in the concatenated string back to its original byte offset in the source file. This is critical — it allows matches found in the normalized view to be traced back to exact positions in the original spec document.

3. **Per-annotation matching**: For each annotation in the group:

   - **Empty quote**: If the annotation has no quote text, it still counts as a reference to the section (using the section's `full_title` as the text slice), but does not contribute to text coverage.

   - **Non-empty quote**: Calls `annotation.quote_range(&contents)` which delegates to `text::find()`.

#### `text::find()` — The Three-Tier Search (`text/find.rs`)

The matching uses a cascading strategy with three attempts, returning the first successful match:

**Tier 1: Exact substring match (raw)**
```rust
fast_find(needle, haystack)
```
A direct `str::find()` — the annotation quote must appear verbatim in the section's concatenated view. This is the fastest and most precise match.

**Tier 2: Exact match after whitespace normalization**
```rust
NormalizedSearch::new(needle, haystack).find(fast_find)
```
Both the needle (annotation quote) and haystack (section text) are whitespace-normalized via `text::whitespace::normalize()`. This collapses all runs of whitespace (spaces, tabs, newlines) into single spaces and trims. The `NormalizedSearch` struct maintains an `offset_map` (a `Vec<usize>`) that maps each byte position in the normalized haystack back to the original haystack. After finding a match in normalized space, the offsets are mapped back and trailing whitespace is trimmed.

**Tier 3: Fuzzy match after whitespace normalization**
```rust
NormalizedSearch::new(needle, haystack).find(fuzzy_find)
```
Uses `triple_accel`'s SIMD-accelerated Levenshtein distance search with an edit distance threshold of 1. This tolerates a single character insertion, deletion, or substitution between the normalized needle and haystack. The result is tagged as `Kind::Fuzzy` (vs `Kind::Exact` for tiers 1 and 2).

#### From Match Range to References

When a match is found, `annotation.quote_range()` returns a `(Range<usize>, Kind)` — the byte range within the `View`'s concatenated string. The `View::ranges()` method then translates this range back through the `byte_map` to produce one or more `Slice`s pointing into the original specification source file. Each slice becomes a `Reference`:

```rust
Reference {
    target: Arc<Target>,       // which spec
    text: Slice,               // exact position in the spec file
    annotation: AnnotationWithId,  // which source annotation matched
}
```

A single annotation quote can produce multiple `Reference`s if the matched range spans across non-contiguous lines in the original spec (since the View joins lines with spaces, a match can bridge line boundaries).

### 5. Coverage Computation

After all references are built, `Report::exec` groups them into `TargetReport`s (one per specification target) and calls `StatusMap::populate()` (`report/status.rs`).

#### `StatusMap::populate()`

This computes per-`Spec`-annotation coverage using byte-offset overlap:

1. **Partition references**: `Spec`-type annotations go into a `specs` map (keyed by annotation ID). All other types (`Citation`, `Test`, `Exception`, `Implication`, `Todo`) go into a `coverage` map (keyed by byte offset).

2. **For each spec annotation**: Record every byte offset it covers. Then scan the `coverage` map for any non-spec references that overlap the same byte range. Each overlapping reference is recorded by type.

3. **Compute completeness** (`SpecReport::finish()`):
   - Start with all spec offsets as "incomplete"
   - **Exceptions** remove offsets (automatically mark as complete)
   - **Implications** remove offsets (automatically mark as complete)
   - **Citations ∪ Tests** remove offsets (an offset covered by either a citation or test is considered complete)
   - The remaining offsets are the `incomplete` count

The result is a `Spec` struct per spec-annotation with counts of total spec offsets, incomplete offsets, and offsets covered by each annotation type.

### 6. CI Enforcement

`ci::enforce_source()` (`report/ci.rs`) performs a simpler line-level check:
- Every "significant" line (any line referenced by any annotation) must have a citation
- Every cited line must have a test
- Exceptions and implications count as both cited and tested

### 7. Output Formats

The `ReportResult` (containing all `TargetReport`s) is passed to one or more output writers:

| Format | Module | Description |
|--------|--------|-------------|
| HTML | `report/html.rs` | Embeds a React frontend with the JSON data |
| JSON (v1) | `report/json.rs` | Legacy JSON format |
| JSON (v2) | `report/json_v2.rs` | Newer format with line-level segmentation and per-segment coverage |
| Snapshot | `report/snapshot.rs` | Human-readable text format for diffing in CI |
| LCOV | `report/lcov.rs` | Standard LCOV coverage format |

## Key Data Flow Diagram

```
Source Files ──→ comment::tokenizer ──→ comment::parser ──→ Annotation[]
                                                                │
                                                    ┌───────────┴───────────┐
                                                    ▼                       ▼
                                          annotation::reference_map   annotation::specifications
                                                    │                       │
                                                    ▼                       ▼
                                          AnnotationReferenceMap      SpecificationMap
                                           (target,section)→annos     target→Specification
                                                    │                       │
                                                    └───────────┬───────────┘
                                                                ▼
                                                    reference::build_references
                                                                │
                                                    ┌───────────┴───────────┐
                                                    ▼                       ▼
                                              section.view()         annotation.quote_range()
                                              (concatenate lines     (text::find 3-tier search)
                                               with byte_map)              │
                                                    │               ┌──────┴──────┐
                                                    ▼               ▼             ▼
                                              View + byte_map   Exact match   Fuzzy match
                                                    │               │             │
                                                    └───────────┬───┘─────────────┘
                                                                ▼
                                                          Reference[]
                                                     (spec position + annotation)
                                                                │
                                                                ▼
                                                    StatusMap::populate()
                                                    (byte-offset overlap analysis)
                                                                │
                                                                ▼
                                                         ReportResult
                                                                │
                                                    ┌───┬───┬───┼───┐
                                                    ▼   ▼   ▼   ▼   ▼
                                                  HTML JSON Snap LCOV CI
```

## Summary of the Matching Algorithm

The matching between annotations and specifications operates at the **byte-offset level within specification section text**. The key insight is that coverage is not line-based — it tracks which exact bytes of a specification section are "covered" by annotations. This allows precise measurement even when a single line contains multiple requirements or when an annotation quote spans part of a line.

The three-tier search (exact → whitespace-normalized exact → fuzzy Levenshtein) provides resilience against minor formatting differences between the annotation quote and the specification text, while preferring exact matches when possible.
