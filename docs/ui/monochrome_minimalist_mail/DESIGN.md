---
name: Monochrome Minimalist Mail
colors:
  surface: '#131315'
  surface-dim: '#131315'
  surface-bright: '#39393b'
  surface-container-lowest: '#0e0e10'
  surface-container-low: '#1c1b1d'
  surface-container: '#201f22'
  surface-container-high: '#2a2a2c'
  surface-container-highest: '#353437'
  on-surface: '#e5e1e4'
  on-surface-variant: '#c4c7c8'
  inverse-surface: '#e5e1e4'
  inverse-on-surface: '#313032'
  outline: '#8e9192'
  outline-variant: '#444748'
  surface-tint: '#c6c6c7'
  primary: '#ffffff'
  on-primary: '#2f3131'
  primary-container: '#e2e2e2'
  on-primary-container: '#636565'
  inverse-primary: '#5d5f5f'
  secondary: '#c6c6cf'
  on-secondary: '#2f3037'
  secondary-container: '#45464e'
  on-secondary-container: '#b4b4bd'
  tertiary: '#ffffff'
  on-tertiary: '#303033'
  tertiary-container: '#e4e1e5'
  on-tertiary-container: '#656467'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e2e2e2'
  primary-fixed-dim: '#c6c6c7'
  on-primary-fixed: '#1a1c1c'
  on-primary-fixed-variant: '#454747'
  secondary-fixed: '#e2e1eb'
  secondary-fixed-dim: '#c6c6cf'
  on-secondary-fixed: '#1a1b22'
  on-secondary-fixed-variant: '#45464e'
  tertiary-fixed: '#e4e1e5'
  tertiary-fixed-dim: '#c8c6c9'
  on-tertiary-fixed: '#1b1b1e'
  on-tertiary-fixed-variant: '#47464a'
  background: '#131315'
  on-background: '#e5e1e4'
  surface-variant: '#353437'
typography:
  headline-xl:
    fontFamily: Geist
    fontSize: 28px
    fontWeight: '600'
    lineHeight: 34px
    letterSpacing: -0.03em
  headline-xl-mobile:
    fontFamily: Geist
    fontSize: 22px
    fontWeight: '600'
    lineHeight: 28px
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Geist
    fontSize: 20px
    fontWeight: '600'
    lineHeight: 26px
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Geist
    fontSize: 15px
    fontWeight: '600'
    lineHeight: 20px
    letterSpacing: -0.01em
  body-lg:
    fontFamily: Geist
    fontSize: 15px
    fontWeight: '400'
    lineHeight: 24px
    letterSpacing: -0.01em
  body-md:
    fontFamily: Geist
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 20px
    letterSpacing: 0em
  body-sm:
    fontFamily: Geist
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
    letterSpacing: 0em
  label-md:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: -0.01em
  label-sm:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 14px
    letterSpacing: 0.02em
  code-badge:
    fontFamily: JetBrains Mono
    fontSize: 10px
    fontWeight: '600'
    lineHeight: 12px
    letterSpacing: 0.04em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  gutter: 0.75rem
  gutter-compact: 0.5rem
  margin: 1rem
  margin-mobile: 0.75rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 0.75rem
  space-lg: 1rem
  space-xl: 1.5rem
---

## Brand & Style

The design system embodies a purist, distraction-free email experience engineered for sustained cognitive focus and lightning-fast navigation. Inspired by the strict utility of Linear, the precision-driven keyboard workflows of Superhuman, and the austere typographic purity of iA Writer, the interface treats communication as an art of disciplined clarity.

### Personality & Demographics
- **Target Audience:** Executives, engineers, researchers, and digital minimalists processing high-volume communications who favor low-latency performance, structural symmetry, and absolute focus over decorative ornamentation.
- **Tone:** Methodical, sharp, authoritative, and frictionless.
- **Emotional Response:** Quiet competence, radical clarity, and tactile velocity.

### Visual Style
The design system operates on an uncompromising, high-density monochromatic framework. Elevation is achieved through hairline structural grids rather than fuzzy dropshadows; spatial depth is communicated via subtle surface tone shifts and razor-thin borders. Visual feedback relies on precise foreground-to-background inversions and crisp monospace micro-indicators, transforming the interface into an extension of thought.

## Colors

The design system utilizes an absolute monochromatic spectrum. Color is intentionally eliminated to prioritize raw content, typography, and density, ensuring that emails and triage states remain legible in both ultra-low light and intense daylight.

### Palette Architecture

#### Dark Mode (Primary Default)
- **Canvas Base (`bg-canvas`):** `#000000` (Pure OLED black)
- **Surface Elevation 1 (`bg-surface`):** `#09090B` (Root interface background)
- **Surface Elevation 2 (`bg-subtle`):** `#111111` (Selected row, active panel, inputs)
- **Surface Elevation 3 (`bg-muted`):** `#181818` (Hover states, badge backgrounds)
- **Hairline Borders (`border-subtle`):** `#222222` (Grid separations, split panes, inputs)
- **Hairline Accent Borders (`border-strong`):** `#3F3F46` (Focus outlines, active selection rings)
- **Text Primary (`fg-primary`):** `#FFFFFF` (Headings, primary sender, unread titles)
- **Text Secondary (`fg-secondary`):** `#A1A1AA` (Body excerpts, message snippets)
- **Text Muted / Metadata (`fg-muted`):** `#71717A` (Timestamps, keyboard hints, disabled states)

#### Light Mode (Secondary Variant)
- **Canvas Base (`bg-canvas`):** `#FFFFFF` (Pristine paper white)
- **Surface Elevation 1 (`bg-surface`):** `#FAFAFA` (Sidebar, peripheral panels)
- **Surface Elevation 2 (`bg-subtle`):** `#F4F4F5` (Selected rows, active inputs)
- **Surface Elevation 3 (`bg-muted`):** `#E4E4E7` (Hover states, badge backgrounds)
- **Hairline Borders (`border-subtle`):** `#E4E4E7` (Structural dividers)
- **Hairline Accent Borders (`border-strong`):** `#A1A1AA` (Focus rings, high-contrast borders)
- **Text Primary (`fg-primary`):** `#09090B` (Sharp ink black)
- **Text Secondary (`fg-secondary`):** `#52525B` (Email thread excerpts)
- **Text Muted / Metadata (`fg-muted`):** `#A1A1AA` (Timestamps, hotkey legends)

### Usage Rules
Never introduce chromatic accent colors (e.g., blue for links or red for destructive badges) unless directly demanded by critical destructive confirmation modal dialogues. Even then, an inverse contrast block (solid white fill with black text) is preferred over red.

## Typography

The typographic hierarchy couples **Geist**—an ultra-clean neo-grotesque sans engineered for developer environments and high-density screens—with **JetBrains Mono** for structural indexing, timestamps, and hotkey indicators.

### Principles
- **Unread vs. Read Contrast:** Message items in lists rely strictly on font weight and foreground tone: unread messages use `Geist SemiBold (600)` with `#FFFFFF` (dark) or `#09090B` (light), while read messages transition immediately to `Geist Regular (400)` with `#A1A1AA` (dark) or `#52525B` (light).
- **Proportional Tabular Alignments:** `JetBrains Mono` is applied to all numerical data (timestamps, counter counts, thread depths) to ensure uniform column widths during row selection.
- **Optical Kerning:** Generous negative tracking (`-0.01em` to `-0.03em`) on titles tightens visual rhythm, while positive tracking (`0.02em` to `0.04em`) on labels and shortcut badges prevents compact mono letters from collapsing.

## Layout & Spacing

The design system employs a three-pane fluid column model with fixed boundaries and hairline borders, optimized for dense keyboard navigation.

### Structural Layout Model
1. **Nav Bar / Rail (Fixed 56px or 200px collapsed/expanded):** Pinned icons, mailbox folders, system status.
2. **List Pane (Fixed 360px - 440px wide):** Dense chronological thread list; no cards, only border-separated rows with fixed heights (40px compact, 48px standard).
3. **Reading / Composition Canvas (Fluid):** Markdown-first reading pane filling the remaining viewport width, bound to a maximum line-length of 720px for optimal reading comprehension.

### Breakpoints & Adaptive Reflow
- **Desktop (>1024px):** Full 3-pane workflow (Navigation, List, Thread Reader). Keyboard shortcuts `j`, `k`, `e`, `r` navigate without cursor interaction.
- **Tablet (768px - 1023px):** Collapsible 2-pane mode (Sidebar hides into slide-over sheet; List and Thread split 50/50).
- **Mobile (<767px):** Single-pane stacked navigation. List view fills 100% viewport width; tapping an email transitions fully to the Reader pane with bottom-sheet command controls.

## Elevation & Depth

This system intentionally rejects blurry ambient drop shadows, soft colored glows, and heavy skeuomorphic dimensionality. Depth is constructed purely through:

1. **Hairline Outlines (`1px solid border`):** Panels, active dialog boxes, and floating command palettes (`Cmd+K`) sit on a precise `1px solid #222222` perimeter (dark mode) or `#E4E4E7` (light mode).
2. **Tonal Planar Surfaces:** 
   - Base plane: `#000000` (Canvas)
   - Secondary pane: `#09090B` (Reading area)
   - Selected / Active element: `#111111`
   - Floating overlays: `#09090B` with `backdrop-filter: blur(12px)` at 92% opacity.
3. **Negative Elevation (Inverted Focus):** Active keyboard selections highlight the target row with a `1px solid #FFFFFF` or `1px solid #3F3F46` border and an subtle internal fill of `#181818`, avoiding any blur spill into neighboring rows.

## Shapes

The geometric architecture balances surgical sharpness with subtle micro-softening (`roundedness: 1`).

- **Base Components (Inputs, Row Highlights, Popovers):** `0.25rem` (`4px`). Retains architectural precision without feeling aggressively harsh.
- **Command Overlays & Dialogs (`rounded-lg`):** `0.5rem` (`8px`) for detached central palettes (e.g., Command Palette, Compose Modal).
- **Badges & Shortcut Tags:** Strict Pill-shape (`9999px`) or exact rectangular tags with `2px` corners. Shortcut chips (e.g., `⌘K`) feature a `2px` micro-radius to resemble mechanical keyboard caps.

## Components

### Buttons
- **Primary:** Solid `#FFFFFF` fill with `#000000` text (dark mode) or solid `#000000` fill with `#FFFFFF` text (light mode). Font: `Geist SemiBold`, 13px. Hover: 90% opacity, no shift.
- **Secondary / Ghost:** Transparent background with `1px solid #222222` outline. Text in `#FFFFFF`. Hover: background fills to `#181818`.
- **Keyboard Action Buttons:** Integrated mono hotkey badge positioned on the right edge (e.g., "Send [ ⌘↵ ]").

### Shortcut Badges & Micro-Caps
- Monospaced `JetBrains Mono` at `10px` or `11px`.
- In dark mode: `#181818` fill, `#71717A` text, enclosed by a crisp hairline border (`#27272A`).
- Padding: `2px 5px`. Height: `18px`.

### Email List Items (Rows)
- Strict single or double-line container without card styling.
- Separated by `1px solid #181818`.
- Height: `40px` (dense) or `48px` (standard).
- Layout: Unread pip (4px circular dot or blank space) | Sender (120px fixed) | Subject (truncated) | Snippet (dimmed `#71717A`) | Time (`JetBrains Mono`, aligned right).
- Keyboard Selected State: Active border or left `2px` white indicator line with background `#111111`.

### Command Palette (`Cmd+K`)
- Center-pinned floating modal, max width `560px`.
- Background: `#09090B` at 96% opacity with `1px solid #27272A` hairline border.
- Instant search input: no background, zero border, font size `16px`, no native focus ring.
- Results list: single-keystroke action shortcuts pinned rightward on each item row.

### Form Inputs & Compose Surface
- Inputs: Flat `#000000` background with a continuous `1px solid #222222` border. Focus state elevates border to `#FFFFFF` without glow or ring spread.
- Textarea / Draft Canvas: Clean distraction-free surface. Metadata headers ("To", "Subject") are minimal single-line borders that dissolve into clean typography.

### Checkboxes & Selectors
- `14px x 14px` square with `2px` border radius.
- Unchecked: `1px solid #3F3F46`, transparent center.
- Checked: Solid `#FFFFFF` fill with `#000000` hairline checkmark.