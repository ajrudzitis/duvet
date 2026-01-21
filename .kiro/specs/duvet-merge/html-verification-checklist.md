# HTML Verification Checklist for Duvet Merge

## Test File Location
`/tmp/duvet-merge-test.html` (681KB) - **ALREADY OPENED IN YOUR BROWSER**

## Generated From
- `integration/merge-three-reports-html/pkg1/report.json`
- `integration/merge-three-reports-html/pkg2/report.json`
- `integration/merge-three-reports-html/pkg3/report.json`

## Expected Content

### Specifications
The merged report should display **2 specifications**:
1. **Example Specification** (https://example.com/spec)
   - Section: "Requirements" (section-1)
   - Should show merged requirements from all 3 packages
2. **Second Specification** (https://example.com/spec2)
   - Section: "Additional Requirements" (section-1)
   - From pkg3 only

### Annotations
The merged report should contain **5 unique annotations**:
1. `pkg1/src/lib.rs:10` - SPEC (MUST) - appears in pkg1 and pkg2 (deduplicated)
2. `pkg1/src/test.rs:20` - TEST - from pkg1
3. `pkg2/src/impl.rs:15` - CITATION - from pkg2
4. `pkg3/src/feature.rs:25` - EXCEPTION - from pkg3
5. `pkg3/src/other.rs:30` - SPEC (SHOULD) - from pkg3

### Merged Status Counts
For the first annotation (pkg1/src/lib.rs:10):
- **spec**: 2 (1 from pkg1 + 1 from pkg2)
- **incomplete**: 2 (1 from pkg1 + 1 from pkg2)
- **related**: Should reference the test annotation

### Links
- **blob_link**: "https://github.com/example/monorepo/blob/main" (consistent across all 3 packages)
- **issue_link**: "https://github.com/example/monorepo/issues" (only in pkg3)

## Verification Steps

### ✓ Basic Functionality
- [ ] Page loads without errors (check browser console for JavaScript errors)
- [ ] Title displays: "Compliance Coverage Report"
- [ ] No blank/white page
- [ ] React app initializes successfully

### ✓ Navigation
- [ ] Left sidebar drawer opens/closes with menu button
- [ ] Sidebar shows 2 specifications
- [ ] Clicking on specification names navigates to spec view
- [ ] Clicking on section names navigates to section view
- [ ] Browser back/forward buttons work correctly

### ✓ Specifications Display
- [ ] "Example Specification" is listed
- [ ] "Second Specification" is listed
- [ ] Each specification shows stats table with counts
- [ ] Stats show correct totals (merged from all packages)

### ✓ Requirements Table
- [ ] Requirements table displays with proper columns
- [ ] Section column shows section IDs
- [ ] Level column shows MUST/SHOULD/MAY
- [ ] Status column shows completion status
- [ ] Comment column shows annotation text
- [ ] Table is sortable by clicking column headers
- [ ] Pagination works (if more than 25 rows)

### ✓ Merged Status Verification
- [ ] First annotation shows merged counts (spec: 2, incomplete: 2)
- [ ] Status colors are correct (red for incomplete, green for complete, etc.)
- [ ] Related annotations are linked correctly

### ✓ Links and References
- [ ] blob_link is present and correct
- [ ] Clicking on source file references works (if blob_link is valid)
- [ ] issue_link is present (from pkg3)
- [ ] Annotation tooltips show on hover
- [ ] Clicking annotations opens detail modal

### ✓ Detail Modal
- [ ] Clicking on an annotation in the spec text opens a modal
- [ ] Modal shows annotation details (type, source, comment)
- [ ] Modal shows related annotations
- [ ] Modal has "Copy" buttons for citation/test/etc formats
- [ ] Modal closes when clicking outside or close button

### ✓ Styling and Layout
- [ ] Material-UI theme is applied correctly
- [ ] Colors match duvet's standard palette
- [ ] Responsive layout works (try resizing browser)
- [ ] Tables are readable and properly formatted
- [ ] No layout overflow or broken CSS

### ✓ Data Integrity
- [ ] All 5 annotations are present
- [ ] No duplicate annotations (pkg1/src/lib.rs should appear once, not twice)
- [ ] Annotation IDs are remapped correctly (0-4)
- [ ] Status counts are additive (not overwritten)
- [ ] Related annotation references point to correct IDs

## Known Issues to Check
- [ ] Verify annotation deduplication worked (pkg1/src/lib.rs appears in both pkg1 and pkg2)
- [ ] Verify status merging worked (counts should be summed, not replaced)
- [ ] Verify related IDs were remapped correctly
- [ ] Verify both specifications are accessible

## Browser Compatibility
Test in at least one modern browser:
- [ ] Chrome/Chromium
- [ ] Firefox
- [ ] Safari
- [ ] Edge

## Performance
- [ ] Page loads in < 2 seconds
- [ ] Navigation is responsive
- [ ] No lag when opening/closing sidebar
- [ ] Table sorting is fast

## Console Errors
Check browser developer console (F12 or Cmd+Option+I) for:
- [ ] No JavaScript errors
- [ ] No React warnings
- [ ] No 404 errors for resources
- [ ] No CORS errors

## Final Verification
- [ ] HTML file is self-contained (no external dependencies)
- [ ] File size is reasonable (~681KB)
- [ ] Can be opened directly from filesystem (file://)
- [ ] Can be served from web server

---

## How to Verify

1. **The file is already open** in your browser at `/tmp/duvet-merge-test.html`

2. **Check console**: Press F12 (or Cmd+Option+I on Mac) to open developer tools

3. **Navigate**: Click through specifications and sections in the sidebar

4. **Inspect data**: Look at the requirements tables and verify counts

5. **Test interactions**: Click on annotations, open modals, sort tables

6. **Report results**: Note any issues or confirm everything works correctly

## Quick Visual Check
If you see:
- A page with "Compliance Coverage Report" title
- A hamburger menu icon in the top left
- A sidebar that opens showing specifications
- Tables with requirement data
- No JavaScript errors in console

Then the HTML is working correctly!
