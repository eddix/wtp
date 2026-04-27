# WTP GUI Design System

`wtp-gui` does not use Zed's full `ui/theme` stack. Instead, it keeps a small local design system on top of bare `gpui`, so layout, spacing, and visual hierarchy stay consistent without pulling in a much larger dependency surface.

## Principles

This system is meant to make four design principles cheap to enforce:

- Contrast: text, surfaces, states, and emphasis should have predictable differences.
- Repetition: buttons, list rows, sections, and page shells should reuse the same primitives.
- Alignment: layout should come from shared helpers and shared spacing tokens, not ad hoc `px(...)`.
- Proximity: related content should live in the same section primitive, with consistent inner and outer spacing.

## Layers

### 1. Tokens

File: [theme.rs](/Users/bytedance/codes/github.com/eddix/wtp/wtp-gui/src/components/theme.rs)

Owns:

- Surface colors: `surface_0/1/2/3`
- Border colors: `border`, `border_soft`
- Text colors: `text_primary/secondary/tertiary/inverse`
- Semantic accents: `accent`, `success`, `warning`, `danger`, `info`
- Typography tokens: `ui_font`, `ui_font_medium`, `mono_font`, `ui_text_size`, `title_text_size`, `status_text_size`
- Geometry tokens: `button_radius`, `panel_radius`, `input_radius`
- Layout tokens: `inline_gap`, `section_gap`, `page_gap`, `heading_gap`, `panel_padding`, `page_padding`
- Shell sizes: `status_bar_height`, `titlebar_height`, `sidebar_width`, `content_min_width`, `content_max_width`

Rule:

- Views should not invent new spacing ramps or color names when an existing token already expresses the same meaning.

### 2. Layout Helpers

File: [layout.rs](/Users/bytedance/codes/github.com/eddix/wtp/wtp-gui/src/components/layout.rs)

Owns:

- `h_flex()`
- `v_flex()`
- `LayoutExt::{h_flex, v_flex}`

Rule:

- Any container using flex semantics should use `h_flex` / `v_flex`, not hand-written `.flex().flex_row()` or `.flex().flex_col()`.

### 3. Primitives

File: [primitives.rs](/Users/bytedance/codes/github.com/eddix/wtp/wtp-gui/src/components/primitives.rs)

Owns:

- Core controls: `Button`, `Icon`, `ListItem`, `badge`, `status_bar`, `panel`
- Page primitives: `page_stack`, `page_intro`, `page_header`
- Section primitives: `section_stack`, `section_intro`, `section_title`
- Shell primitives: `titlebar_brand_segment`, `titlebar_context_segment`, `sidebar_section_label`, `nav_item`
- Form primitives: `field_header`, `field_block`, `toggle_row`
- Content primitives: `info_block`, `info_card`, `empty_hint`, `empty_state`, `empty_state_with_action`, `message_banner`, `key_value_row`, `stat_card`
- Utilities: `middle_truncate`

Rule:

- Once a repeated pattern exists here, views should consume it instead of rebuilding the same structure locally.

### 4. Facade

File: [design_system.rs](/Users/bytedance/codes/github.com/eddix/wtp/wtp-gui/src/components/design_system.rs)

This file is the explicit import surface for the local system. It re-exports the approved tokens, layout helpers, and primitives so the rest of the app has a single place to point at when the system grows.

## Page Composition Standard

For normal right-panel pages, prefer this shape:

```rust
page_stack()
    .child(page_header(...))
    .child(section_stack(1).child(section_intro(...)).child(...))
    .child(section_stack(1).child(section_title(...)).child(...))
```

Inside a section:

- Header and description: `section_intro(...)`
- Simple title only: `section_title(...)`
- Label and technical hint: `field_header(...)`
- Label, hint, and control: `field_block(...)`
- Read-only path/code/value: `info_block(...)`
- No data: `empty_hint(...)`

For prominent empty sections, prefer this shape:

```rust
empty_state(...)
empty_state_with_action(..., Button::new(...))
```

For app chrome, prefer this shape:

```rust
v_flex()
    .child(titlebar_brand_segment(...))
    .child(titlebar_context_segment(...))
    .child(sidebar_section_label(...))
    .child(nav_item(...))
```

## Current Component Inventory

Already standardized:

- Buttons: `Primary`, `Secondary`, `Ghost`
- List rows: selected and hover states
- Status bar
- Titlebar brand and context segments
- Sidebar navigation rows and section label
- Message banners
- Inputs: 32px single-line mono input
- Empty states with optional primary action
- Field blocks
- Toggle rows
- Page headers
- Section containers
- Read-only info blocks
- Key/value metadata rows
- Stat cards

Not standardized yet:

- Dropdown
- Tree
- Tabs
- Modal/dialog
- Toast/notification component
- Table/data grid

## Linter Scope

The lint entry point is [gpui_layout_lint.rs](/Users/bytedance/codes/github.com/eddix/wtp/wtp-gui/tests/gpui_layout_lint.rs).

Today it already enforces GPUI flex helper correctness. It should gradually also enforce the low-ambiguity subset of this design system, especially:

- Prefer `h_flex` / `v_flex`
- Prefer semantic spacing tokens in views
- Prefer `page_stack` / `section_stack` for obvious page and section containers

The linter should only cover rules with low false-positive risk. Visual judgment still belongs in review.

## Adoption Rule

When adding a new UI pattern:

1. Check whether a token or primitive already exists.
2. If the pattern appears in 2 or more places, promote it into `primitives.rs`.
3. Only after that, wire a lint rule if the usage can be checked mechanically.

That order matters. Otherwise the linter ends up enforcing conventions that do not yet have a clean abstraction.
