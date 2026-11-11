# Nexus design and workflow review November 4 2026

## Review outcome

The current prototype demonstrates connected functionality, but its visual design and user experience do not meet the owner's standard for Nexus. A substantial UI and UX redesign is required. Custom code, working persistence and passing tests do not establish product acceptance.

Ease of use is a core product differentiator: Nexus should make billing and everyday care operations clearer, faster and less frustrating. Clinical, billing and executive users need distinct experiences shaped around their actual work, while sharing connected records and a coherent brand.

The target remains November 13, 2026, with all planned features in scope and most external integrations lower priority. This review adds significant design and workflow work; it does not establish a revised delivery estimate. Two-way calendar synchronization is now an explicit integration requirement, with sequencing still to be decided.

The initial checkpoint requested documentation and factual answers. Later on November 4, the owner authorized a Grok research fleet, durable Codex context and orchestration guidance. The updated sequence is **functional workflows first, then the branding implementation pass**. The observations below distinguish clear direction from examples and unresolved choices; the [dated research and planning review](../research/research-planning-review-2026-11-04.md) records the follow-up.

## Brand and visual direction

Nexus should feel calm, grounded and connected to nature, with a growth theme. The current dark interface is too saturated and too uniformly green. The desired direction is a softer, more passive earthy green supported by lighter colors and a broader neutral palette.

Create coordinated light and dark palettes rather than deriving one by simply inverting the other. Some colors may work across both modes: for example, an earthy green could become a heading color in light mode. The final shades, contrast relationships and usage rules have not been selected.

Develop a distinctive logo mark, supporting assets and thoughtful brand guidelines before restyling every screen. Explore custom typography and iconography where they improve identity without creating unnecessary dependency, loading or maintenance problems. Heading letter spacing currently feels too tight and needs deliberate treatment within the type system.

Explore subtle background texture or nature-inspired patterns so large surfaces do not feel like flat color blocks. The supplied reference uses restrained points and connecting lines. Its relevance is the subtle texture and sense of connection, not a decision to use its blue palette or copy its exact artwork. Nature icons, branching paths and growth motifs are directions to explore, not finalized designs.

![Owner supplied reference for a subtle patterned background](review-assets/2026-11-04-background-reference.png)

## A Nexus component system

Build a considered Nexus component library and design language. The current implementation already uses custom React components and CSS; the missing work is a coherent, high-quality system and consistent application of it.

- Replace harsh white outlines around dark-mode groups with more intentional grouping and surface hierarchy.
- Revisit corner radii, borders, spacing, typography and component proportions together.
- Redesign buttons so they feel precise and distinctive. Button appearance and press behavior are useful brand touchpoints.
- Design dropdowns, menus, popovers and other controls as part of the system. Current dropdown styling and opening behavior feel messy and poorly integrated with the page.
- Define hover, keyboard focus, pressed, selected, visited where applicable, disabled, loading, success and error states. These states should feel intentional across the application.
- Use restrained custom motion, such as a small button-press response, to give the product character. Avoid heavy effects or large movements that distract from work; support reduced-motion preferences.
- Use custom JavaScript where it improves an interaction or communicates state. Additional code is a means to a better experience, not a goal by itself.

The architectural preference remains to limit unnecessary dependencies and own the product's workflows and design. Local speech and model services fit that preference. This does not mean every underlying library or model must be invented from scratch: the present local services use whisper.cpp and llama.cpp/Qwen, without requiring hosted OpenAI inference for those workflows.

## Forms and record creation

The current forms are frustrating to complete and need more than a cosmetic restyle. Once the brand and component foundations are established, review each form and record-creation flow individually.

For each flow, examine field order, grouping, terminology, required information, defaults, selection controls, validation, error recovery, confirmation and preservation of entered work. Review how the form opens and fits into its surrounding page, especially dropdowns and dialogs. Apply the resulting patterns consistently without assuming every clinical, billing or administrative form should use the same layout.

The acceptance question is whether a person can complete the real task comfortably and understand what happens next, not whether the form contains every database field.

## Profile navigation and account controls

Remove the theme selector from its current prominent header position. Place appearance preferences inside a proper profile menu at the **bottom left, beneath the other navigation options**. This incorporates the owner's correction of the initial bottom-right description. The full profile/menu experience still needs to be designed and built.

Settings and available actions must reflect the viewer's permissions: administrators should have broader controls, while other users see the options relevant to their role. Hiding a menu item must match actual server authorization.

Leaving the current account or switching local demo operators is not sufficiently discoverable. The existing implementation has an **Exit workspace** button at the bottom of the left sidebar, but its presence does not resolve the usability problem. Provide a clear account identity, exit/sign-out action and appropriate account-switching behavior in the eventual profile experience.

## People and contextual profile cards

When a person is referenced, use a recognizable person representation, with their name and profile/avatar treatment, rather than relying on a bare text link. Develop a reusable contextual profile card inspired by the usefulness of a Sleeper player card, adapted to Nexus.

The card's information and actions should depend on both who is viewing it and whose profile is being viewed. The owner's examples are illustrative, not a complete permission matrix:

- An executive viewing a clinician could see relevant operational information and actions to message them or inspect their schedule.
- A patient viewing their therapist could see their own sessions/history, permitted contact information and relevant resources.

Explore a compact hover/focus preview on desktop, with a click opening the person's full page. On mobile, a tap could open a bottom sheet. Keyboard, touch and assistive access need equivalent paths; hover alone cannot be required to reach information. The exact fields and actions will be specified later.

## Statuses, timelines and audit visibility

Stages, statuses and metadata need a more expressive, consistent visual vocabulary. Explore dedicated status colors with emojis or custom icons and clear labels. Color and symbols should reinforce meaning rather than replace it.

History sections should look and read like timelines or logs, with a clear sequence, date/time, actor, action and relevant context. Explore a branching/tree treatment where it helps explain related events or versions. This reinforces the owner's growth and nature direction, but chronology must remain easy to follow.

Preserve the visible audit-trail messaging. The owner values clear communication that changes are tracked and logged. Retain this reassurance while ensuring the wording accurately describes what the application records.

## Layout, information density and page language

Reduce unnecessary pills, containers and nested sections. Use deliberate breathing room, while correcting large empty areas that add no value. The area beneath the task/insight bar described during the review is a specific example to revisit.

The client pages may contain appropriate domain information, but the owner is not yet confident about their density or arrangement. Review what each role needs to scan first, what belongs in progressive disclosure and what deserves a separate view. Do not assume that a large amount of information is either inherently wrong or justified merely because it appeared in research.

Page-opening taglines and headers need review. It is unresolved whether the problem is the specific copy, the prominence of these statements or the use of taglines on operational pages at all. Compare alternatives before making that a blanket removal decision.

## Role experiences and workflows

### Clinical and task workflows

Reassess the complete task flow, not just the task list's appearance. Review task creation, assignment, priority, due dates, status changes, related records, follow-up and completion in the context of a person's workday.

Clarify how onboarding establishes a user's role, default home and available views. The current ability to switch among Clinical, Billing and Executive views is useful for an administrator's demonstration, but it should not imply that every person must choose among three applications at each visit.

### Billing

Billing requires a dedicated research and workflow-design phase. The owner does not consider the current page an adequate billing experience; it feels too much like a clinical workspace reused with financial labels.

Investigate what the intended centers actually charge for, how services become charges, which staff own each step, what prevents billing, how exceptions are resolved and what reconciliation requires. Use that understanding to design billing-specific queues, terminology, information hierarchy and actions. Preserve connection to clinical records without copying the clinician's workflow.

### Executive

Executive needs its own focused discovery and design work. The current screen does not communicate operational meaning clearly enough and is not accepted as the intended leadership experience.

Metrics need understandable labels, context, useful visualization and a clear next action. A bare ratio such as **2/12** is not sufficient at a glance. Show what the numerator and denominator represent, the relevant population/time context and how the user can investigate or act. The exact visual treatment should follow the brand and leadership workflow work.

## Two way calendar synchronization

Add calendar integration to the requirements. Google Workspace is the owner's initial example:

- Pull connected calendar events into Nexus.
- Allow appropriate edits from Nexus to update the source calendar.
- Send appointments created in Nexus to the person's connected calendar.

This is two-way synchronization, not a one-time export or read-only calendar overlay. Before implementation, decide which calendars/events are in scope, event ownership, what information is shared, matching/deduplication, conflict handling, recurrence, cancellations, time zones and disconnected/error behavior. The integration is requested; provider selection beyond the Google example and delivery priority remain open.

## Current implementation answers

| Question | Verified answer as of this review |
|---|---|
| Is a premade UI kit such as shadcn/ui being used? | No premade component system was found in the current dependencies, imports, assets or component source. Runtime frontend packages are React and React DOM; controls, inline SVG icons and styles are in the repository. This confirms implementation structure, not independent invention of every generic shape or pattern. See [package.json](../../frontend/package.json) and [ui.tsx](../../frontend/src/ui.tsx). |
| Does Nexus Walkthrough indicate support for multiple workspaces? | It is the name of the synthetic demonstration organization, displayed in the sidebar. It is not an organization switcher. The Workspace dropdown changes Clinical/Billing/Executive presentation within the same organization. Whether the target customers need multi-organization switching remains a product question. The overloaded word workspace contributes to confusion. |
| Does Refresh reset anything? | It reloads authorized records/permissions and the program catalog from the local API. It does not delete, reset or reseed saved data. The main workspace reloads after its own successful changes and on entry/operator changes, but currently has no general live push/background refresh. Other sessions' changes can be picked up with Refresh. Whether a visible refresh action belongs in the final product remains undecided; automatic freshness and clear status should be designed deliberately. See [App.tsx](../../frontend/src/App.tsx). |
| How do I leave this account and view the others? | In the updated local build, open the profile at the bottom left beneath navigation and choose **Switch or leave workspace** to return to the local operator picker. This clears the selected local operator, not the database. It is not authenticated sign-out; production identity remains deferred. Its discoverability needs improvement. |
| Why can this account switch all three views? | The walkthrough administrator is allowed all three. Ordinary roles receive role-appropriate defaults; additional views depend on effective permissions. Changing a view never grants access. First-run setup creates an administrator, initially shown the Clinical view. Staff roles are assigned in Team and access. A complete personalized onboarding/invitation/authentication journey is not implemented. See [workspace.ts](../../frontend/src/workspace.ts) and [Setup.tsx](../../frontend/src/Setup.tsx). |
| Is the client page grounded in research? | It is informed by the repository's competitor/product research and earlier Figma facesheet requirements. Those sources do not establish that every current field placement or the current density works for real users. There is no documented observed staff-usability study validating this page. See [research](../research/nexus-market-and-product-research.md) and [Figma source inspection](figma-source-inspection.md). |
| Can the xAI key on this Mac be used? | Yes. A nonempty credential was successfully retrieved from the specific macOS Keychain service used by the supplied project's runner. It was captured privately in memory and not displayed or saved. It can be supplied to a worker process when that workflow is selected. At the initial observation checkpoint, no xAI network call or paid request had been made, so provider acceptance, credits and account/model permissions were not yet tested. Later authorized worker activity is recorded in the dated research review. The earlier environment-only check did not establish absence from Keychain. |

## Orchestration and older implementations

The owner supplied RoadRunner's `CLAUDE.md` as a reference for a future Nexus orchestration file adapted to ChatGPT, Codex and OpenAI. Preserve the useful planner/delegation/verification structure, but adapt it to Nexus's actual repository, available tools, model choices, task boundaries and cost controls when the owner decides how to proceed.

The attached file is reference material, not an instruction to activate its Claude-specific roles, model defaults, Grok executor rules, spending exceptions or unrelated project procedures. That initial documentation pass did not activate a Nexus orchestration policy. The owner subsequently authorized the [Codex/OpenAI + Grok workflow](../development/orchestration.md), which now governs this research/readiness work.

The owner permits moving older demos or implementations when useful. No move or deletion was requested for this pass. When reorganizing later, preserve the relevant work and history and keep the active implementation unambiguous.

## Proposed order for the next design work

The later November 4 instruction changes delivery order: first research the workflows and make the application function correctly, then implement the visual brand. Within that later design pass:

1. Establish the brand direction, light/dark palettes, typography, logo and icon direction, with restrained nature/growth motifs.
2. Define the component system, interaction states and motion; resolve account/profile navigation and the meaning of workspace.
3. Research and design the clinical/task, billing and executive journeys separately, including information hierarchy and understandable metrics.
4. Review forms, record creation, dropdowns, profile cards, statuses and timelines one flow at a time using those foundations.
5. Specify calendar synchronization and test the connected role journeys on desktop and mobile.

Keep the [capability ledger](acceptance-ledger.md) and [November 13 plan](../development/prototype-target-2026-11-13.md) connected to this review. The present visual design is not approved, and functional completion must not be used as a substitute for the requested UX work.
