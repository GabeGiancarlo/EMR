# Paid Grok test: what makes a good Nexus logomark

November 4, 2026. This is a reviewed synthesis of an actual xAI research response, not a generated logo or an approved brand direction.

## Request receipt

| Field | Observed value |
|---|---|
| Endpoint | `POST https://api.x.ai/v1/responses` |
| Model | `grok-4.7`, verified in the authenticated model list |
| Credential | Keychain service `xai-api-key`; xAI name `nvim-grok` |
| Team check | API metadata matched the exact team in the owner's console link |
| Started | November 4, 2026, 4:02:49 p.m. Eastern |
| Completed | November 4, 2026, 4:07:46 p.m. Eastern |
| Response ID | `1ce05b78-b5d4-9292-a122-83f6d807f7ee` |
| Provider-reported charge | **$0.927428** |
| Raw cost field | `cost_in_usd_ticks: 9274280000` |
| Usage | 313,333 input tokens; 18,100 output tokens, including 9,608 reasoning tokens; 32 web-search calls |
| Result | Complete five-section research report, 30,332 characters after parsing |

xAI documents the cost field as the per-request billed amount, including token/tool charges and applicable discounts; 10 billion ticks equal one dollar. This verifies a paid generation response. It does not establish that the console UI has displayed the same total or reconcile interrupted requests. [xAI cost tracking](https://docs.x.ai/developers/cost-tracking).

The earlier smoke test was only an authenticated model-list request. Earlier research responses include both usable reports and failed/incomplete outcomes; those are distinguished in the [run ledger](xai-run-evidence.md). The console browser was signed out, so no account-wide billing screen was verified. No credential was printed, written to a file or included in a prompt.

The API output concatenated two interim JSON progress messages and the final JSON report. The runner initially flagged a schema error; inspection recovered the complete final object, and a parser fix now validates the last report while preserving the raw response. This recovery made no additional paid call. Raw evidence remains in ignored `.local/xai/logomark-paid-test.response.json`; the full worker report is `.local/xai/logomark-paid-test.md`.

## Main finding

A strong Nexus mark should remain recognizable in a tiny browser tab, compact navigation and a monochrome billing document. Start with a clear silhouette and one idea: connected work. Growth can be a secondary reading, supported by pattern and motion elsewhere. The mark should not require green, a gradient, fine leaf detail or animation to be recognized.

This is design judgment informed by identity and interface guidance, not a measured finding that one style is universally best. Muted green can work when its luminance and surrounding colors are suitable; there is no evidence that the owner's earthy-green preference is inherently unsuitable for a small mark.

## Concepts to compare

| Concept | Idea | What to test |
|---|---|---|
| Junction N | Two firm paths meet at a distinctive junction, suggesting an N and connected workflows | Avoid a generic N monogram; keep the junction recognizable at 16px |
| Aperture | An open geometric shape suggests access and a passage between workstreams | Avoid resemblance to a checkbox, window or common app icon |
| Branch junction | A simple stem/node structure hints at growth without detailed foliage | Preserve a distinct silhouette; ensure it does not become a generic wellness sprig |

The worker favored Junction N and Aperture. The coordinator retains the branch concept as a legitimate option because nature/growth is explicit owner direction. No actual collision search or user test justifies ranking these concepts by legal safety. The existence of a trademark design-code category is a search aid, not evidence that a category is riskier.

Keep the wordmark separate from the symbol. Explore horizontal and stacked lockups, a symbol-only variant and a wordmark-only variant. Avoid forcing every concept into one mark. Custom typography is optional; a carefully chosen and properly licensed existing face can provide a strong foundation.

## Evaluation scorecard

Score actual sketches, not descriptions. Suggested weights are a design decision rather than a scientific recognition model.

| Criterion | Weight | Evidence to collect |
|---|---:|---|
| Small-size recognition | 25% | 16/20/24/32px examples at actual size |
| One-color and print reproduction | 20% | Black, reversed white, grayscale and small PDF header |
| Distinctiveness | 20% | Comparison against relevant existing identities; no copying |
| Fit to Nexus | 15% | Does it suggest connected, precise, humane work without needing a long explanation? |
| Adaptability | 10% | Light/dark surfaces, compact nav, document exports and app icon |
| Optional supporting expression | 10% | Pattern/motion strengthen the system while the static mark still works |

Use a brief recognition exercise with representative people: show each candidate, introduce a delay and compare recognition/confusion in realistic contexts. Record the protocol and results. The worker's proposed two-second distractor test is an exploratory method, not a validated universal acceptance threshold.

## Deliverable and validation brief

Prepare a master vector symbol, wordmark/lockups, monochrome/reversed variants, optical small-size variant if needed, favicon/app-icon files, PDF/print examples, clear-space/minimum-size guidance and provenance/license records. Define the geometry first; supporting vegetation patterns should stay away from dense data and small critical marks.

Test on actual warm light and charcoal dark surfaces, not just pure white and black. Check whether internal gaps disappear, strokes blur or the icon resembles an ordinary control. Print a billing header in grayscale and verify its surrounding document text separately. A recognition result at large presentation size is insufficient.

Browser favicons, search-result favicons and installed-app icons have different delivery requirements. The current Google Search guidance lists supported raster formats and recommends a source larger than 48px; use a supported search asset and recheck requirements at release. An SVG browser icon is a separate choice. [Google Search favicon guidance](https://developers.google.com/search/docs/appearance/favicon-in-search).

Logo-specific accessibility exceptions do not exempt ordinary brand-colored text, focus indicators or controls. Use an understandable accessible name for a functional logo link, such as “Nexus home,” and suppress decorative pattern announcements. Prefer a clearly visible logo presentation even where an exception applies. [W3C contrast guidance](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html), [W3C image alternatives](https://www.w3.org/WAI/tutorials/images/).

If a motion variant is explored, make its final frame identical to the static mark, keep it brief and optional, and provide a reduced-motion presentation. Repeated growth animation beside a clinical or billing task would compete with the work. The static identity must be complete before motion is added.

## Originality and clearance boundaries

Assess the name and symbol separately and together. Trademark confusion concerns overall commercial impression and related goods/services, not merely identical pixels. A new drawing does not clear the word “Nexus.” Screening should extend beyond a single federal keyword search to relevant design searches and other sources of existing use. This research did not perform that clearance. [USPTO likelihood of confusion](https://www.uspto.gov/trademarks/search/likelihood-confusion), [USPTO clearance-search guidance](https://www.uspto.gov/trademarks/search/comprehensive-clearance-search-similar-trademarks).

Do not imitate protected emblems or assume that any generic medical symbol is automatically available. The worker's blanket statement that every medical-cross mark is legally prohibited was too broad and has not been adopted. Likewise, neither this report nor the $1,500 research authorization creates a requirement to hire counsel before sketching or authority to purchase design services.

The existing `frontend/public/nexus.svg` has not been accepted as the final identity; its provenance remains to be checked. Next useful design work is a small comparison of these concepts in actual Nexus navigation and document contexts, after the functional work reaches the agreed branding stage.
