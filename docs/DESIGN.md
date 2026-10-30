# Iceberg design direction

Reading this as: a desktop now-playing instrument for Windows listeners, in a glacial field-instrument language, dial ENERGY 1 / RHYTHM 2 / MOTION 1.

## Identity

Iceberg is a quiet instrument for music that is already playing. It is not a media hub, a library, or a dashboard. It shows the current line and gets out of the way.

The visual language is a field instrument: cold mineral surfaces, a hairline rule, a numbered index, and one warm signal mark. The "iceberg" idea lives in restraint and depth, not in a literal mountain logo. Most of the product is below the surface: tray only, no window when idle, no account.

The wordmark is the identity. It is set in Bahnschrift. There is no generated logo mark.

## Dials

- ENERGY 1: the product says hello quietly. No glow, no floating cards, no decorative motion.
- RHYTHM 2: the widget and settings share the same rules, but the settings sheet breaks groups with hairlines instead of repeating one card.
- MOTION 1: hover and state transitions only. The progress bar steps, it does not run a continuous animation. No animation runs while paused or hidden.

## Palette

| Token | Value | Role | Reason |
|---|---|---|---|
| ink | `#0C1012` | dark surface | Cold, near-black mineral base. Reads as an instrument, not a media card. |
| bone | `#ECEAE1` | primary text on ink | Warm off-white holds up over a translucent surface without glare. |
| slate | `#8A9694` | secondary text, hairlines | Desaturated mineral grey keeps hierarchy without a second accent. |
| signal | `#F07A4F` | active lyric, progress, focus | A rescue signal on ice. The one warm mark, so the current state is unmistakable. |
| paper | `#F3F1EA` | light settings surface | A paper field manual, not a white SaaS panel. |
| vermilion | `#B23F26` | signal on paper | The same rescue mark, darkened for contrast on a light surface. |
| danger | `#E0685A` | destructive actions | Semantic only. Never used for decoration. |
| live | `#6FA88A` | real playing state | Semantic only. A small square status mark, no glow, no pulse. |

Rules:

- Every screen uses at most two core surfaces, one primary text, one secondary text, and the signal. Green and red are semantic exceptions.
- No gradients as color treatment. Theme swatches are two solid fields, not gradients.
- Album color is opt-in and uses one extracted hue. It never overrides the signal on first run.
- Default themes must pass 4.5:1 for normal text and 3:1 for large text.

## Typography

| Role | Typeface | Reason |
|---|---|---|
| Display, section titles, widget title, wordmark | Bahnschrift | A condensed technical grotesk that belongs on a Windows instrument. It gives Iceberg a voice that Segoe UI alone does not. |
| Body, labels, descriptions, lyrics | Segoe UI Variable | Native Windows text, excellent at small sizes, no web font cost. |
| Timecodes, section numbers, diagnostic values | Cascadia Mono | Tabular numbers for time and memory data. Mono is functional here, not a costume. |

No uppercase labels with wide tracking. Headings are sentence case. Hierarchy comes from size and weight.

## Surface and shape

- Radius scale: 2px, 4px, 6px. Tags and swatches use 2px. Controls use 4px. The widget shell uses 6px. Nothing is a pill.
- Borders are 1px hairlines derived from the text color. They define the surface without a shadow.
- Shadows are off by default. The widget shadow remains an explicit opt-in setting.
- The widget stays translucent because it sits over the desktop. That is its reason. It does not use CSS backdrop blur; native acrylic and mica remain opt-in.
- Settings are a single sheet. Groups are separated by a 1px rule. There is no card per group.

## The widget

- Compact instrument strip. Small square cover with a 2px radius, title, artist, mono duration, and a 2px progress rule.
- A single waterline rule separates now playing from lyrics. A 10px signal segment sits at its left edge. This is the product motif, repeated in the settings index.
- The title is Bahnschrift. The artist is Segoe UI at 0.7 opacity. The active lyric is signal. Inactive lyrics are bone at 0.7. Past lyrics are bone at 0.62.
- The static equalizer is removed. The play or pause glyph and the active line already show state.
- Loading and error states are text with a small square signal mark. No spinner loop.
- The widget does not add timers. It keeps the existing one timer for the next lyric, the stepped progress update, and the fade timer.

## The settings sheet

- A 188px index spine with the wordmark, six numbered sections, and a now-playing status line.
- The active section is marked by the signal number and the label weight. No filled pill and no colored side stripe.
- The content is one continuous sheet. Each group has a Bahnschrift heading and a hairline. Controls sit in rows, not cards.
- Diagnostics use one readout: private memory is the large value, the rest is a mono ledger. No row of equal stat cards.
- The About block is a footer, not another card.
- Theme choices are two-column rows with a two-field swatch. The selected theme gets a signal square, not a glow.

## Accessibility

- Every focusable control gets a 2px signal outline with a 2px offset. No `outline: none` without a replacement.
- Targets are at least 32px high in the desktop settings window. The 44px mobile target rule applies to the marketing site, not this fixed desktop window.
- The settings loading state, error state, and diagnostics error state are explicit. No silent defaults.
- The widget's contrast is measured against the composited surface. Default themes pass over white, black, and mid-grey desktops.
- Reduced motion disables transitions. The karaoke underline is the only per-line transform, and it stops when paused.

## Performance budget

- No new dependencies.
- No backdrop filters, no web fonts, no icon package, no React.
- CSS only for the redesign. Svelte changes stay within the existing reactive model.
- No new timers or intervals. The settings diagnostics interval already exists and runs only on the Advanced tab.
- Keep the 3 to 5 recycled lyric nodes and the stepped progress bar.
- The redesign must not regress the section 1 budgets: idle under 15 MB, visible under 70 MB, CPU near zero when paused.
