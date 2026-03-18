# A11y Audit Fixes

## Branch: `feature/a11y-audit-fixes`

## Changes Summary

### CRITICAL (3 fixed)
1. **app.css** — Added `:focus-visible` outlines, `prefers-reduced-motion`, fixed contrast (#666 → #595959)
2. **TimeseriesChart / SingleAxisChart** — Added `role="img"` + `aria-label` to chart containers
3. **ColumnMappingDialog** — Focus trap, Escape to close, `aria-labelledby`, focus first element on open

### MAJOR (all fixed)
4. **FileList** — Space key on role="button", aria-label on remove button, file item label
5. **DeviceSelector** — Space key, aria-label on device items, `aria-hidden` on decorative dot
6. **AnnotationPanel** — Space key, aria-labels on delete/edit/offset buttons, color input labels, `aria-hidden` on color dot
7. **+page.svelte** — `role="status"` on loading, `role="alert"` on error, `id="main-content"` skip target
8. **app.html** — Skip link to main content
9. **app.css** — `--text-secondary` contrast improved from ~3.5:1 to 4.6:1

## Files Modified
- `src/app.css`
- `src/app.html`
- `src/routes/+page.svelte`
- `src/lib/components/Chart/TimeseriesChart.svelte`
- `src/lib/components/Chart/SingleAxisChart.svelte`
- `src/lib/components/ColumnMapping/ColumnMappingDialog.svelte`
- `src/lib/components/Layout/FileList.svelte`
- `src/lib/components/Layout/DeviceSelector.svelte`
- `src/lib/components/Annotation/AnnotationPanel.svelte`

## Verification
- svelte-check: 0 errors, 0 warnings
