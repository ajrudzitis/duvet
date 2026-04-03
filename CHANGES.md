# JSON v2 Report Format and Stable Annotation IDs

## Motivation

Duvet generates compliance reports per package, but a single specification is often implemented across multiple packages. Merging reports from independent `duvet report` runs requires two things the existing JSON format lacks:

1. **Roundtrip capability.** The v1 JSON is write-only — it uses streaming macros with no deserialization support. A merge tool needs to read reports back in.
2. **Stable identifiers.** v1 assigns annotation IDs sequentially (`0, 1, 2, …`), so IDs from two independent runs collide. Merging requires IDs that are deterministic from content, not from insertion order.

## What Changed

### Stable Annotation IDs

Each annotation now gets a content-derived 16-character hex ID computed as `FNV-1a(source_path || '\0' || anno_line || '\0' || target_path)`. This composite key is unique within a package (two annotations can't start on the same line in the same file) and deterministic across runs. The ID is added to the existing `AnnotationWithId` struct alongside the legacy sequential `id`.

### JSON v2 Format (`--json-v2`)

A new `json_v2` report module implements a serde-based format that can be both serialized and deserialized. Invoked via `duvet report --json-v2 <path>`.

#### v1 → v2 Schema Comparison

| Aspect | v1 | v2 |
|---|---|---|
| Serialization | Streaming macros, write-only | `serde` derive, roundtrippable |
| Annotation IDs | Sequential `usize` | Content-derived 16-char hex string |
| Annotations include quote | No | Yes (needed for merge without re-parsing sources) |
| Version field | None | `"version": "2.0"` |
| Coverage key | `"statuses"` keyed by integer | `"coverage"` keyed by stable ID |
| Related annotations | Integer IDs | Stable string IDs |
| Line status | Fixed 256-entry refs table, segments reference by index | Direct `u8` bitmask per segment (bits 0–5: coverage flags, bits 6–7: level) |
| Line segments | Compact arrays `[[ids], status_id, text]` | Named objects `{ annotation_ids, status, text }` |

#### v2 Top-Level Schema

```json
{
  "version": "2.0",
  "blob_link": "...",
  "issue_link": "...",
  "specifications": { "<spec_path>": { "title", "format", "sections": [...] } },
  "annotations": [{ "id", "source", "blob_link", "target_path", "quote", "type", ... }],
  "coverage": { "<stable_id>": { "spec", "incomplete", "citation", "test", ..., "related": [...] } }
}
```

Key structural decisions:
- Annotations carry their `quote` text, making reports self-contained for merge.
- `coverage` uses stable string IDs so entries from different packages can be unioned.
- Line segment status is a `u8` bitmask encoding coverage flags and annotation level directly, eliminating the need for a separate refs lookup table.
- Spec lines are pre-segmented with coverage baked in — the same segmentation logic as v1, but using named fields and stable IDs.

### Supporting Changes

- `serde_json` added as a dependency.
- `AnnotationLevel` derives `Deserialize`.
- `TargetReport` fields and `status` module made `pub` for access from `json_v2`.
- `ReportV2` and `LineSegmentV2` use `#[serde(deny_unknown_fields)]` for strict deserialization.
- Integration tests added for all existing test configurations, with snapshot files tracked via Git LFS.

## What Didn't Change

The existing v1 JSON format, HTML report, snapshot format, and LCOV output are untouched. The v2 format is additive — it's a new output path that coexists with v1.
