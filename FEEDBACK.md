# Code Review Feedback

## Defects / Correctness Issues

1. **Stable ID uses `target_path()` but `AnnotationV2` uses `resolve_target_path()` — potential mismatch across packages** (annotation.rs:128, json_v2.rs:457)

   `stable_annotation_id()` hashes `annotation.target_path()` (the raw, unresolved path), but `AnnotationV2.target_path` is populated with `annotation.resolve_target_path()` (which canonicalizes relative file paths). If two packages reference the same spec via different relative paths (e.g., `../specs/rfc.md` vs `specs/rfc.md`), they'll get different stable IDs even though they refer to the same spec. Since the stated motivation is cross-package merging, this seems like a design gap. At minimum, document the decision; ideally, hash the resolved path.

2. **`LineV2` untagged enum has ambiguous deserialization** (json_v2.rs:90-96)

   `#[serde(untagged)]` with `Plain(String)` first means serde will try to deserialize any JSON string as `Plain`. That's fine. But a JSON array of objects will try `Plain` first, fail, then try `Segmented` — this works but produces poor error messages on malformed input. More importantly, an empty array `[]` will deserialize as `Segmented(vec![])`, which is semantically different from `Plain("")`. The round-trip PBT may not catch this because bolero generates typed values, not arbitrary JSON. Consider using an internally-tagged or adjacently-tagged representation for robustness, or at least document this behavior.

3. **`build_specification_v2` emits duplicate lines for multi-line slices** (json_v2.rs:586-611)

   The loop `for lineno in slice.line_range()` iterates once per line number, but on each iteration it uses `slice.to_string()` (the *entire* slice text, which may span multiple lines) and `slice.range().start` (the offset of the *first* line). This means a 3-line slice produces 3 identical `LineV2` entries, each containing the full multi-line text with the same offset. The v1 JSON has the same pattern, so this may be an inherited issue, but it's worth flagging since v2 is a new format and an opportunity to fix it.

4. **`stable_id_map` is populated twice with potentially different values** (json_v2.rs:444-492)

   The first loop (line 448) computes `stable_annotation_id(annotation)` from `report.annotations`. The second loop (line 489) reads `reference.annotation.stable_id` from `AnnotationWithId`. These should produce the same value, but the code doesn't assert that. If they ever diverge (e.g., due to a bug in `reference_map`), the `HashMap::insert` silently overwrites. Either assert consistency or remove the redundant second population.

## Design / API Concerns

5. **`pub mod json_v2` and `pub mod status` are broader than needed** (report.rs:19,23)

   These modules are made `pub` but nothing outside `duvet/src/report/` imports them. The `TargetReport` fields are also made `pub`. If the intent is to expose these for future external tooling, that's fine, but it should be called out. If it's just to satisfy the `json_v2` module's access, `pub(crate)` would be more appropriate and avoids committing to a public API surface.

6. **`read_report_v2` / `read_report_v2_from_reader` are `#[allow(dead_code)]`** (json_v2.rs:682,696)

   These are the "roundtrip" half of the format — the key differentiator from v1 per the motivation. But they're unused and `allow(dead_code)`. This means the roundtrip capability isn't actually exercised in any integration path. The PBT tests exercise serde roundtrip in-memory, but there's no test that writes a report and reads it back via the file I/O path. Consider adding at least one integration test that reads back a generated v2 report, or remove the `allow(dead_code)` and wire up a `--merge` flag or similar.

7. **FNV-1a hash collision risk is undocumented** (annotation.rs:97-109)

   FNV-1a 64-bit has a birthday-bound collision probability of ~1 in 2^32 for ~77k annotations. That's probably fine for any realistic project, but the CHANGES.md claims IDs are "unique within a package" without qualification. A brief comment noting the probabilistic nature would be appropriate. Also, consider whether a merge tool should detect and handle collisions.

8. **`AnnotationType` in v2 doesn't include `IMPLEMENTATION`** (json_v2.rs:168-180)

   The internal `AnnotationType::Citation` is parsed from both `"implementation"` and `"citation"` strings (annotation.rs FromStr). The v2 format serializes it only as `"CITATION"`. If existing users have `type=implementation` annotations, the v2 output will silently rename them to `CITATION`. This is technically correct (they're the same variant internally) but could confuse users comparing v1 and v2 output. Worth a note in the schema docs.

## Code Quality / Style

9. **Missing blank line before `fn fnv1a_64`** (annotation.rs:93)

   There's no blank line between the closing brace of `reference_map` and the doc comment for `fnv1a_64`. Every other function in this file has a separating blank line.

10. **`is_zero` helper could be a method** (json_v2.rs:19-21)

    The free function `fn is_zero(v: &usize) -> bool` is used only for `skip_serializing_if`. This is a common serde pattern, but placing it as a module-level function with a generic name is a bit loose. Consider making it a const or at least marking it `#[inline]`.

11. **Comment in `segment_line` is stale/misleading** (json_v2.rs:427-428)

    The comment says "we could return Plain, but the design says to return Segmented if there were refs." But the function already returns `Plain` early if `refs.is_empty()` (line 389). If refs are non-empty but none cover the line, you still get `Segmented` with a single segment at status_id 0. This is a valid design choice but the comment reads like a TODO rather than a deliberate decision.

12. **Integration test config has a stale comment** (report-json-v2.toml:2)

    `# Tests Requirements 8.1, 8.2: --json-v2 flag generates v2 JSON report` — these requirement numbers appear to reference an internal spec/task list that isn't part of the repository. Remove or replace with something meaningful to an outside reader.

13. **Snapshot files are all 3-line LFS pointers** (integration/snapshots/*_json_v2.snap)

    Every `_json_v2.snap` file is an LFS pointer, which means reviewers can't inspect the actual snapshot content in the diff. This is consistent with how `_json.snap` files are handled, but it means the actual v2 output format is unreviewed. Consider including the `report-json-v2_json_v2.snap` content (the dedicated test) inline rather than in LFS, since it's small and purpose-built for this feature.

## Summary

The core design is sound — serde-based roundtrippable format with content-derived IDs is a clear improvement over v1. The main concerns are: (1) the `target_path()` vs `resolve_target_path()` inconsistency in the stable ID, which could undermine the cross-package merge use case; (2) the multi-line slice duplication bug inherited from v1; and (3) the roundtrip read path being dead code with no integration coverage. The rest is polish.
