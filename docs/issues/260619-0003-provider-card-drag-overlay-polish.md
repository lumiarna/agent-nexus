# Provider card drag overlay polish

## What to build

Add a `DragOverlay` to the Provider card `DndContext` so the dragged card renders as a floating copy that follows the pointer, instead of the current behaviour where the source card stays in flow with only `opacity-60` and a border highlight. This is a UX polish item, not a functional gap — ordering already works.

## Why

With `rectSortingStrategy` on an `auto-fill minmax(300px,1fr)` grid, reordering causes cards to swap positions instantly. There is no visible "lift" or insertion indicator, so it can feel like nothing is happening until the drop. A `DragOverlay` rendering the card contents at pointer position gives clear drag feedback and is the idiomatic @dnd-kit pattern for grid sorting.

## Acceptance criteria

- [ ] While a Provider card is dragged, a `DragOverlay` renders a visual copy of that card at the pointer position.
- [ ] The source card in the grid shows a placeholder (reduced opacity / dashed border) during drag.
- [ ] The overlay card preserves the same content, status badge, and quota bars as the source.
- [ ] Keyboard / touch sensors remain usable (overlay should not break the existing `PointerSensor` activation constraint).
- [ ] Card equal-height logic (`cardMinHeight` via `ResizeObserver`) still works with the overlay in place.

## Out of scope

- No changes to Project or Sync drag — they use `verticalListSortingStrategy` and already feel adequate.
- No sortable keyboard coordinates tuning unless a separate a11y issue is filed.

## Notes

The overlay content can reuse `SortableProviderCard`'s children render-prop, or a lightweight `ProviderCardBody` extracted from the current inline JSX. Extracting `ProviderCardBody` would also reduce duplication between the grid card and the overlay.
