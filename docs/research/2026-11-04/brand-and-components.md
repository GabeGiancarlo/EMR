# Nexus brand and component research brief

Reviewed November 4, 2026. Synthesized from actual Grok `brand-foundation-stream` research, current frontend source and coordinator review. This is a proposed direction and implementation brief. No new logo, palette, component library or visual redesign is approved or implemented by this document. Functional workflows come first.

## Owner direction and current evidence

Nexus connects clinical care, billing and executive oversight. The brand should feel calm, precise and grounded, with restrained references to growth and connection. The owner rejects the current saturated green surfaces, white outlines, soft corners, tight heading spacing, weak buttons, awkward menus and forms, excess empty containers and unclear dashboard labels. The profile/settings/theme/exit menu belongs at the **bottom left beneath navigation**.

The frontend already owns its React/CSS/inline-SVG components. `frontend/package.json` does not include shadcn/ui, Radix or a CSS component framework. The task is to create a coherent, tested Nexus system and apply it consistently. A dependency count alone says nothing about usability.

`frontend/src/styles.css` contains repeated green surfaces, negative heading tracking and several generous empty-state blocks. `frontend/src/theme.css` defines a separate dark theme rather than a filter inversion. Existing focus and reduced-motion rules are useful foundations, but they have not been comprehensively audited. `ThemeControl.tsx` supports light, dark and system preferences and is now mounted inside the bottom-left local profile menu. Production account management and the broader component redesign remain work.

The existing `frontend/public/nexus.svg` file is real. Its provenance, ownership and suitability as the final mark remain unverified. The owner's supplied connected-dot reference is about quiet texture and connection, not a requirement to copy its blue colors or artwork.

## Three directions to explore

| Direction | Character | Strength | Risk |
|---|---|---|---|
| Grove | Leaves, seeds, green fields and softer forms | Makes growth immediately visible | Repeats the current excess green/softness and can look like a generic wellness brand |
| Field ledger | Warm neutral surfaces, muted earth accents, precise geometry, restrained branching | Connects care and growth with financial clarity | Needs disciplined detail to avoid becoming beige and anonymous |
| Instrument | Mostly neutral, highly ordered, minimal nature beyond the mark | Clear dense operational screens | Can resemble a generic administrative tool |

**Proposed starting direction: Field ledger, using Instrument's restraint on data surfaces.** This is a recommendation to explore, not an owner decision. Nature should appear in distinctive small details: a mark, occasional background structure, an audit branch, a short confirmation response. It should not compete with clinical facts or financial exceptions.

## Exploratory tokens

These are candidate values from the worker brief, not a validated accessible palette. Measure every actual foreground/background/state combination before adoption. Brand green must not replace distinct success, warning, error and informational semantics.

| Token | Light candidate | Dark candidate |
|---|---|---|
| Page background | `#f4f1ea` | `#141816` |
| Main surface | `#fbfaf6` | `#1c2420` |
| Secondary surface | `#efeae2` | `#24302b` |
| Primary text | `#1c2824` | `#e7eee9` |
| Secondary text | `#3e5148` | `#c5d4cc` |
| Decorative divider | `#d5d0c6` | `#3c4a44` |
| Brand accent | `#3f5c45` | `#8faf96` |
| Text on accent | `#f7f6f2` | `#13201b` |
| Focus candidate | `#8a5a12` | `#f0d7a4` |

Explore 2px control corners, 4px panels and 6px dialogs as an initial geometric system. These are candidates, not a requirement that all controls have the same outline. Use a small spacing scale such as 4/8/12/16/24/32, but let real content and task hierarchy determine layout. Remove empty containers that exist only to fill a dashboard. Audit headings at actual screen sizes rather than globally tightening tracking.

Status appearance must be defined per domain. “Inactive,” “discharged” or “reversed” is not automatically an error; the meaning depends on the workflow. Each state needs a clear word and a consistent icon, with color as reinforcement. The owner's emoji idea can inform tone, but critical status should not depend on platform-dependent emoji rendering.

## Component system to build

| Component family | Contract |
|---|---|
| Buttons | Clear primary/secondary/destructive hierarchy; readable hover, focus, pressed, disabled and busy states; prevent duplicate consequential actions |
| Fields and forms | Persistent labels, useful defaults, grouped questions, inline validation, preserved work after errors and clear save state |
| Selects/comboboxes | Use native behavior where it fits; custom searchable choices need keyboard navigation, typeahead, announced state and correct portal/viewport placement |
| Menus/popovers | Predictable anchor, no page expansion surprises, dismiss behavior, focus return and accessible names |
| Status/metadata | Domain-specific label/icon/color combinations; no unexplained tiny counters or color-only dots |
| Person references | Name and identity context, role-aware preview and a clear destination; desktop hover has a focus/click equivalent; mobile can use an accessible bottom sheet |
| Timelines | Ordered events with actor, action, timestamp/time zone, object and correction linkage; a branch motif may decorate but cannot replace chronology |
| Profile menu | Bottom-left placement; identity/role, appearance, permitted settings and explicit exit/sign-out action; current local operator exit must not be described as authenticated logout |
| Tables/work queues | Clear next action and responsible person, meaningful sorting/filtering, scope and loading/empty/error states |
| Dialogs/sheets | Sensible initial focus, labeled close, keyboard dismissal where appropriate, unsaved-work handling and visible validation |

Person cards must distinguish staff, clients and providers. An executive clicking a clinician may see authorized schedule/contact actions; a client viewing a therapist may see their own sessions, resources and contact information. A staff reference does not always open a patient chart. Server-side scope applies to the preview as well as the destination. Do not preload hidden sensitive details and merely conceal them with CSS.

Every interactive component should cover default, hover, focus-visible, pressed, selected/expanded where applicable, disabled, loading, success and error. Visited styling is relevant to navigation links, not all controls. Define touch behavior as deliberately as pointer behavior.

## Accessibility and motion

Target WCAG 2.2 AA, with selected additional practices. Contrast, keyboard operation, meaningful names, readable errors and non-color cues must be validated in actual workflows; a token table cannot prove conformance. [W3C WCAG 2.2](https://www.w3.org/TR/WCAG22/).

AA pointer target size is 24×24 CSS pixels subject to the criterion's exceptions. A 44×44 product target is useful for important touch actions but is not the universal AA requirement. [W3C target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html).

Hover/focus content needs appropriate dismissal, persistence and pointer access. Focus must remain visible and not be hidden by sticky bars or dialogs. A 2px outline is not automatically sufficient: area, contrast and surrounding states matter. Shadows can form a valid focus indicator when the relevant conditions are met; there is no blanket prohibition on them. [W3C hover/focus content](https://www.w3.org/WAI/WCAG22/Understanding/content-on-hover-or-focus.html), [W3C focus appearance, AAA](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html).

Explore short 120–160ms opacity or small-position changes for press/confirmation feedback. Avoid continuous growing vines, parallax, animated backgrounds behind data and layout movement while entering information. Respect reduced motion, preserving all state information in a static form. Interaction-animation disabling is an AAA criterion Nexus may deliberately adopt. [W3C interaction animation](https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html).

Inactive controls have contrast exceptions in WCAG; keeping them understandable remains a product goal. Do not disguise important blocked actions as illegible disabled controls—explain what prerequisite is missing when useful.

## Typography, icons and assets

Choose a readable UI family before considering a custom typeface. Self-host appropriately licensed font files when selected, keep notices and verify tabular figures, punctuation, numerals and rendering across platforms. DM Sans and Manrope have OFL sources; current CSS references do not prove those files are bundled. A system-font fallback is different from copying Microsoft font binaries into the app. [DM Fonts](https://github.com/googlefonts/dm-fonts), [Manrope license](https://raw.githubusercontent.com/google/fonts/main/ofl/manrope/OFL.txt), [OFL](https://openfontlicense.org/open-font-license-official-text/), [Microsoft font FAQ](https://learn.microsoft.com/en-us/typography/fonts/font-faq).

Audit current inline SVG provenance. A small, consistent in-repo icon set can fit Nexus without an icon runtime dependency, but copied paths still require applicable notices. If Lucide is selected, its license page distinguishes Lucide and Feather-derived material. [Lucide licensing](https://lucide.dev/license).

Required brand deliverables after concept selection: master vector mark, monochrome/reversed/small-size versions, horizontal/stacked wordmark lockups, favicon/app icons, light/dark placement examples, PDF/print header, safe-area/minimum-size guidance, pattern tiles, component tokens, motion examples and an asset provenance/license manifest. Preserve a recognizable silhouette without a gradient or animation. Trademark screening and final clearance are separate tasks, not implied by an attractive design or this research budget.

## Expanded source review

The repository already has `frontend/public/nexus.svg`: a green rounded tile with a cream N and orange dot. That is an existing asset to replace or refine, not evidence of an accepted brand. Current CSS names DM Sans/Manrope, but no bundled font loading was found; appearance can depend on local fonts. Proposed bans on an N, circles, strokes, uppercase or a particular number of shapes are exploratory preferences, not owner requirements.

The current light focus pair `#d4ac70` against `#f7f8f5` calculates to approximately 1.98:1. It deserves a focused contrast correction during component work; this isolated measurement is not a complete accessibility audit. Focus Visible 2.4.7 is AA and Focus Appearance 2.4.13 is AAA. An outline plus shadow is a valid technique when its actual presentation meets the relevant conditions. [W3C Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html), [W3C technique C40](https://www.w3.org/WAI/WCAG22/Techniques/css/C40).

Prepare separate padded maskable app icons and validate cropping/safe areas; a 16px favicon master is not automatically suitable. Reduced-motion mode should use a deliberate static state for continuous activity rather than simply making an infinite animation extremely fast. These are implementation requirements to test in the later component pass, not an approved palette or motion system. [Web app manifest icon masks](https://www.w3.org/TR/appmanifest/#icon-masks).

## Implementation and validation sequence

1. Complete functional contracts and meaningful failure/permission paths; retain this brief in parallel.
2. Evaluate mark concepts and palette/type candidates in real clinical, billing and executive examples.
3. Build tokens and a component specimen page covering every state in both themes and reduced motion.
4. Implement navigation/profile, buttons, fields, choices, statuses and overlays before decorative backgrounds.
5. Rework each form as a task: field order, defaults, validation, focus, preserved work, confirmation and next action.
6. Apply person cards and timeline patterns with server-enforced scope, then add restrained brand assets.
7. Validate keyboard/touch/zoom, contrast, loading/error/empty cases and dense real-size synthetic data. Test print and exported-document marks separately.

The owner has not approved final colors, fonts, radii or a logo. Functionality-first does not impose a new calendar ban on branding, and the research authorization is not permission to purchase fonts or commission outside design work. The next brand decision should compare concrete concepts in context rather than restyle the entire application speculatively.


## Final component review

Candidate text on accent measures 6.87:1 (light) and 6.99:1 (dark) in isolation. Candidate focus against the page measures 5.24:1 / 12.77:1 but only 1.26:1 / 1.71:1 directly against the accent fill; validate the actual offset and surrounding surfaces. Decorative dividers measure about 1.47:1 / 1.71:1 on main surfaces and cannot alone distinguish required controls. Existing light focus against its page is about 1.98:1. These measurements are not a keyboard/workflow conformance audit. WCAG Focus Visible is AA and Focus Appearance is AAA.

The general reduced-motion rule shortens duration without necessarily stopping infinite iteration; use a static busy state, as the refresh override demonstrates. A person-card preview needs server-authorized fields **and** actions for the particular viewer and subject. Theme selection changes a local preference immediately; navigation/operator switching, not theme selection, invoke unsaved-work guards. No palette, mark, typeface or radius is owner-approved by these tests.
