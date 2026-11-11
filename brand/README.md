# Nexus brand kit v0.3

Start with [Nexus-Brand-Guidelines-v0.3.pdf](Nexus-Brand-Guidelines-v0.3.pdf) (29 pages, with contents and an
index), or open [guidelines/index.html](guidelines/index.html) in a browser. The HTML edition is the same
document with live hover states.

Nothing here is wired into the app yet; `frontend/` still uses its old styles and `frontend/public/nexus.svg`.

## What is in it

| Folder | Contents |
| --- | --- |
| `logo/` | Horizontal and stacked lockups, symbol, small-size symbol and wordmark in Pine, Linen and Ink; app tiles; favicon. SVG, with PNG exports in `logo/png/`. |
| `logo/masters/` | The two symbol drawings (full and small-size), the wordmark, and the four build steps shown in the guide, in `currentColor`. |
| `icons/` | 46 icons: one SVG each in `svg/`, a sprite, and `nexus-icons.ts` with the path data and a `NexusIconName` type. |
| `patterns/` | Ring press, ring field, bark, contour, understory, and the leaf vein. Carved SVG; pressed PNG for the ring and bark prints (`-pressed-pine.png` is coloured, `-pressed-mask.png` is an alpha mask for CSS `mask-image`). |
| `illustrations/` | Four cut-paper scenes, each with a day and a night form in one file, and ten motifs. SVG. |
| `tokens/` | `nexus-tokens.css`, `.json`, `.ts`, and `contrast-audit.csv` (72 pairings, all passing). |
| `fonts/` | Fraunces (SemiBold, Regular), Manrope (ExtraBold, Bold) and Source Sans 3, with their OFL licences. `fonts/variable/` holds the variable sources the Fraunces and Manrope cuts are made from. |
| `guidelines/` | Source of the guide: `template.html` and the two product screens as HTML partials. |
| `tools/` | The generators. |

## Using it in the app

```css
@import "brand/tokens/nexus-tokens.css";   /* --nx-* custom properties */
```

Set `data-nexus-theme="dark"` on the root element for the dark theme. Components should use role tokens
(`--nx-action`, `--nx-surface`) and never palette colours directly. The styles under "product UI" and
"the leaf card" in `guidelines/template.html` are the working reference for the sidebar, tables, badges,
buttons, fields and cards. No React components exist yet; the guide's asset-library page lists what to build.

```tsx
import { nexusIcons, type NexusIconName } from "brand/icons/nexus-icons";

<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
  <path fillRule="evenodd" d={nexusIcons[name]} />
</svg>
```

Scenes in `illustrations/` read `--nx-illo-*` custom properties, so inlined SVG follows the theme (moon for sun
at night). Used as an `<img>`, a scene shows its daylight form.

## Rebuilding

Everything is generated. Needs Python 3 with numpy, scipy, scikit-image, matplotlib, Pillow and fontTools; the
PDF and PNG exports also need Google Chrome.

```bash
python3 brand/tools/build_all.py
```

The symbol is Convergence, chosen by the owner on November 11, 2026. To add an icon, add a function to
`tools/build_icons.py` and list it in `ICONS`. To add a page to the guide, add a `<section class="page" id="...">`
to `guidelines/template.html`; `data-sec` and `data-toc` put it in the contents, and the `INDEX` table in
`tools/build_guidelines.py` feeds the index.

## Open

- "Growth, on the record.", the new wordmark and typefaces, how far the leaf card goes, and the client and
  session wording are still the owners' to confirm.
- No trademark search or recognition testing has been done.
- No photography, editable templates (slides, letterhead), executive workspace, charts or phone layouts.
- Motion and hover states are specified in the guide and tokens but not built or tested in the app.
- The style references that informed the kit live outside the repo and are not reproduced here.
