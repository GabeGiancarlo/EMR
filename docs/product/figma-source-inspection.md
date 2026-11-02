# Figma source inspection — November 2, 2026

Read through the connected Figma tools under session `e5e6fe13-5f1a-47a8-ac63-67c48b0c6a20`. Source designs were not edited.

## Prep specification text

The Prep node is an ordinary FRAME containing canvas text, not a native SPEC_BLOCK document. The following complete text-node labels were returned by metadata for [Prep](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081). They are historical design requirements, not proof of implemented behavior or verified compliance rules.

### Node 1:5251

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5251)

1️⃣ Dashboard (Role-based Home) Layout Zones & Components KPI Tile Row Admissions Today / This Week Occupancy % (beds or census) Un-signed Notes Claims Denied (billing view)  Tiles support click-through for drill-down. Task & Alert Feed “My Tasks” list with status pill (Due, Overdue, Completed) Filters: Clinical, Billing, Administrative Bulk-action checkboxes (sign, re-assign, mark done) Upcoming Schedule Mini-Calendar Color-coded by service type (Individual, Group, UA test) Hover shows patient name + link to facesheet “Start Tele-session” button if telehealth enabled Quick-Action Launcher Big buttons: New Note, New Appt., Check Eligibility, Run Report Can pin / reorder per user Insights Panel (executive accounts) Sparklines for Revenue/Day, Length-of-Stay, Outcomes Score Date-range picker → instantly refresh all tiles

### Node 1:5252

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5252)

2️⃣ Patient Overview (Facesheet) Header (persistent while in chart) Photo, legal & preferred name MRN, DOB (age auto-calculates), pronouns Allergies (red badge), Legal Flags (Part 2, Court-Ordered) “Action Icons”: 📄 New Doc, 💊 Prescribe, 📅 Schedule, 🏁 Discharge Tab Strip (vertical or horizontal) Info | Clinical Notes | Treatment Plan | Medications | Labs | Billing | Attachments | Audit Facesheet Body Components SectionFields / WidgetsDemographics CardAddress, phone, email, emergency contact, preferred languageAdmission & Level-of-Care CardProgram, admit date, expected discharge, primary therapist, payer classInsurance & Authorizations CardPayer, member ID, eligibility status pill, current auth #, units remaining, Run Eligibility buttonClinical Snapshot CardDiagnoses (ICD-10/DSM-5 list), current meds, last UA result, vital-signs last takenProgress Timeline (right rail)Vertical timeline of major events: admit, treatment-plan reviews, med changes, claims filed

### Node 1:5256

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5256)

ZoneComponentsTemplate SelectorSearch bar + tags (SOAP, DAP, Group Note, Discharge)Rich Editor PaneWYSIWYG + smart-phrases ({{PATIENT_AGE}})Diagnosis PickerICD/DSM searchable modal; auto-codes claim when savedIntervention LibraryDrag-drop interventions/goals into noteTimer / Compliance MeterDisplays active session length, warns at 45 minSignature PanelE-sign, co-sign flow, supervisor overrideAttachmentsDrag files, audio, images; OCR auto-ingests into noteRevision & Lock BannerState: Draft → Pending Sign → Signed (locked); break-glass unlock logs reason Treatment Plan Editor is often a sub-tab:  – Problems → Goals → Objectives grid with status dropdowns  – Review date & auto-notify 7 days prior  – Participant list (Therapist, Case Manager, Patient) with e-signature slots

### Node 1:5260

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5260)

Current Med List Grid Columns: Med name, Dose, Route, Freq, Start, End, Prescriber Badges: PRN, Controlled, High-Risk Add/Edit Medication Wizard Drug search (FDB / First Databank API) Sig builder (dose, route, frequency, PRN reason) Interaction checker modal (shows conflicts, allergies) E-prescribe submit (SureScripts) + two-factor for Schedule II MAR Timeline Calendar with administration events; color for late/missed Bulk documentation for nursing staff Refill & Renewal Queue Requests list; one-click approve/deny; auto-notify pharmacy/patient

### Node 1:5264

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5264)

ComponentDetailOrder FormTest search (LOINC), priority, collection site, ICD tying medical necessityCollection TrackerStatus chips: Ordered → Collected → Received → VerifiedResults GridValue, units, reference range; critical flags in redTrend GraphSparkline on click; pin multiple labs for overlayReview/Sign WorkflowCheckboxes, bulk sign & notify patient portalCritical Alert BannerPops top-of-screen until acknowledged; writes audit

### Node 1:5267

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5267)

ComponentFunctionReport Library SidebarFolders (Clinical, Billing, Compliance) with saved & shared reportsParameter FormDate ranges, program, payer, diagnosis, custom fieldsVisualization CanvasCharts, tables; drag-to-re-order widgets; auto refreshExport BarPDF, CSV, XLSX, Scheduled EmailRole-Based Saved DashboardsPersonal vs Facility vs Corporate levelCompliance Monitors42 CFR Part 2 access counts, HIPAA breaches, unsigned notes aging

### Node 1:5270

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5270)

User Directory – sortable grid; bulk role assignment Role-Permission Matrix – ACL table with granular rights (view/create/export) Audit Log Viewer – time range, user filter, export PDF; highlights Part 2 entries Policy & Form Repository – version control, expiry reminders Consent Template Builder – e-signature fields, multilingual support Alert Rules Engine – “If UA positive & diagnosis = OUD → alert MD” Facility Settings – treatment levels, billing codes, note templates

### Node 1:5274

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5274)

ZoneComponentsInbox ListUnread, Starred, Patient-linked threadsThread ViewRich text, attachments, audit stamp if added to chartCompose Pane@mentions, quick insert patient linkGroup Channels#Clinical, #Billing, #Admin; searchable historyCo-Sign Request QueueDisplays pending notes needing co-signatureTele-health Mini-AppVideo tile, screen share, whiteboard; auto-save recording to chart

### Node 1:5278

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5278)

Home Dashboard – upcoming appts., meds refills due, balance Appointment Manager – view, confirm, request change Secure Mailbox – message provider, upload docs Documents & Summaries – visit summaries, lab results (after provider release), consent forms Payments – ledger + Pay-Now (Stripe, Elavon) Resources Library – PDFs, videos for recovery education Profile & Settings – contact info, preferred language, MFA configuration

### Node 1:5281

[Open source](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5281)

Facesheet Insurance Tab Policies table (primary, secondary) Real-time eligibility button, auth request link Claims Worklist Filters: Date of service, Payer, Denied, Pending, Paid Row actions: View EOB, Resubmit, Correct Claim Batch select → Export 837P or Send to Clearinghouse Claim Detail Drawer Line items: CPT/HCPCS, Dx pointer, Modifiers, Units, Charge $ Attach documentation button (progress note, auth) Status timeline with ANSI 277 codes Patient Ledger Charges, payments, adjustments, balance “Post Payment” modal, PCI-compliant card tokenization Denial Management Queue Denial code, reason, appeal deadline countdown Templates for appeal letters; attach supporting docs

## Calendar frame

[Clinician v3](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=58-1743) was retrieved with design context and screenshot. It places a categorized task queue beside a large event calendar, with date navigation, Today, Day/Week/Month and an add action. Events carry service color and individual/group context. The visible screenshot uses historical example patients/dates; these must not become application seeds. The user identifies these designs as outdated initial references, so the implementation must retain the workflow structure while using the custom Nexus component system and real authorized records. Current light/dark support, legible labels, full weeks and responsive layouts remain required.

Generated reference code uses absolute positioning and Tailwind classes; it is not the application implementation. No Tailwind or premade component package will be installed. Event/user imagery is data-driven and no example portrait or patient name is imported as application content.
