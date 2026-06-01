# al-tabi (التابي) — Design System

> **Identity codename:** Mihbar (محبَر)
> Arabic-first RTL desktop reporting app. Calm expert advisor for pharmacy distributors and accountants in Libya/MENA.

---

## 1 — What is al-tabi?

**al-tabi** (التابي) is a Windows desktop application (Tauri) that sits on top of an existing ERP — **Marketing2026 on SQL Server** — and turns natural-Arabic business questions into instant reports.

Users type plain Arabic (« كم مبيعات اليوم؟ », « ديون الموردين »), an AI agent translates that into safe read-only SQL, executes against the local ERP database, and returns a result they can read, export (PDF / Excel), save as a reusable query, schedule for recurring runs, or push to a Telegram bot.

It does **not** replace the ERP. It is an **intelligence layer** over it.

### Audience
- **Primary** — owners, accountants, and inventory managers at small-to-mid wholesale pharmacies in Libya/MENA (5–50 employees). Time-pressed, non-technical, decision-oriented.
- **Secondary** — local IT staff who install and connect; Telegram users who consume reports remotely.

### Core surfaces
- **Login** — SQL Server connection (server, db, auth)
- **Reports (تقارير)** — scheduler: create and manage recurring SQL reports
- **Search (بحث)** — generic report catalog from Supabase
- **Notifications (تنبيهات)** — generated alerts log
- **Intelligence (الذكاء)** — main AI chat interface, always mounted
- **Saved (المحفوظات)** — proven queries you can run without AI
- **Settings (الإعدادات)** — SQL connection, Telegram, themes, updates, business profile

### Sources referenced (private to user)
- This brief: `brief/DESIGN_SYSTEM_BRIEF.md`
- Original code paths cited but NOT supplied to this design system:
  - `src/lib/themes.ts`
  - `src/components/ui/futuristic-nav.tsx`
  - `src/components/ui/ai-assistant-interface.tsx`
  - `AGENTS.md`

> ⚠ **Reader note** — the actual codebase was not attached. This system is built directly from the brief and intentionally proposes a **new** visual identity (Mihbar), not the indigo/violet/burgundy themes that exist in the current code.

---

## 2 — Visual Identity at a Glance

| | |
|---|---|
| **Name in Arabic** | التابي |
| **Name in Latin**  | al-tabi |
| **Mark**           | monogram «ت» in deep teal + small copper «sparkline check» badge |
| **Primary color**  | `#0F6E70` — deep teal, signature trust color |
| **Accent color**   | `#B86A2C` — warm copper, used for AI moments |
| **Canvas**         | `#F6F2E8` — aged paper, warm cream |
| **Ink**            | `#0E1729` — deep navy text |
| **UI font**        | IBM Plex Sans Arabic |
| **Display font**   | Reem Kufi (hero Arabic only) |
| **Numeric font**   | IBM Plex Mono (tabular nums in tables) |
| **Direction**      | RTL native |
| **Modes**          | Light (Daylight Ledger) + Dark (Lamplit Ledger) |

---

## 3 — Content Fundamentals

### Voice
**Calm expert advisor.** Concise Arabic. Decision-oriented. Honest when something fails. Reassuring during loads. Never effusive.

### Casing & form
- **Arabic** is the primary language. English appears only for SQL keywords, schema names, brand tokens (e.g. `Telegram`, `SQL Server`), and currency unit `د.ل`.
- Use **MSA / فصحى مبسطة** — no dialect, no Anglicisms (« Tab » → «تبويب»).
- Address the user with respect, never « يا أخي » or familiar address. Use neutral imperative: «اختر»، «أدخل»، «اضغط».
- Sentence-case Arabic (Arabic has no case anyway). Latin labels: Title Case for nav, sentence case for body.

### Pronouns & person
- App refers to itself in **third person** (« التابي يعمل على… »), never « I ».
- User is addressed as **you implicit** (verb-form). Avoid the explicit «أنت» unless emphatic.

### Length
- **Buttons:** 1–2 words. «تشغيل»، «تصدير»، «حفظ»، «جدولة».
- **Toasts:** 1 line for the result + 1 line for the cause/next step. No third line.
- **AI replies:** answer in 1 sentence, then expand if asked. Numbers come with units.
- **Empty states:** state + obvious next action. «لا تقارير مجدولة — أنشئ واحداً».

### Emoji
**Not used as UI.** Forbidden in buttons, headers, toasts, table headers, AI chat. May appear inside user-typed content only (verbatim echo). Status uses colored dots and lucide glyphs instead.

### Specific copy patterns
| Situation | Pattern | Example |
|---|---|---|
| Loading | gerund «جارٍ + verb» | «جارٍ الاتصال بقاعدة البيانات…» |
| Success | past «تم + noun» + count | «تم توليد التقرير — 47 صفاً» |
| Empty   | negation + invitation | «لا توجد نتائج في هذه الفترة» |
| Error   | what failed + what we'll do | «فقد الاتصال — سنحاول بعد 10 ثوانٍ» |
| AI summary | result first, then framing | «بلغت 42,180 د.ل (+8.4% عن أمس)» |

### Forbidden tone
- ❌ Marketing exuberance («رائع!»، «WOW»، multiple exclamation marks)
- ❌ Childish or playful («هيا نبدأ! 🎉»)
- ❌ Raw technical errors («Error 0x80004005») — always translate to user terms
- ❌ Cyberpunk/hacker affectations («accessing matrix…»)

---

## 4 — Visual Foundations

### 4.1 Palette philosophy
Two pillars: a deep **teal** for trust and authority, and a warm **copper** for the AI and human moments. They sit on a **warm paper** canvas rather than cold grey. Dark mode keeps the same identity by inverting only the canvas — copper softens, teal lifts, but the personality is preserved.

We use **two colors maximum on any one surface**, plus semantic states. Never reach for a third hue to spice things up — restraint is the brand.

### 4.2 Typography
- **IBM Plex Sans Arabic** is the workhorse. Weights 400/500/600/700. It has personality where Cairo is flat.
- **Reem Kufi** is the display voice: logo lockup, splash, occasional bold Arabic numerals on dashboards. Never in body copy.
- **IBM Plex Mono** is used wherever digits must align — tables, currency columns, schema names, SQL previews. Always `font-variant-numeric: tabular-nums`.
- Headings sit on `--tracking-tight` for Latin; Arabic gets none.
- Body line-height is generous (`1.55`) because Arabic glyphs are taller than Latin x-height.

### 4.3 Backgrounds
- **No gradients in production UI.** Solids only. (One exception: the bottom-nav active pill carries an inset highlight to suggest depth.)
- **No full-bleed photography.** This is an accounting app — imagery would feel dishonest.
- **No repeating patterns or textures** in chrome. The paper warmth in `--bg-canvas` is texture enough.
- **No illustrations.** Empty states use a single small Lucide glyph in muted ink + a one-line message.

### 4.4 Borders & dividers
- Hairline `1px solid var(--border-subtle)` everywhere — like a ruled ledger. This is the dominant separator, not shadows.
- Inputs and cards get `var(--border-default)`. Focus rings use `var(--shadow-focus)` (3px teal halo, no border color change).
- Tables: only horizontal rules. No vertical lines except the optional inner-border for sticky columns.

### 4.5 Shadows
Warm-tinted, soft, conservative. Four-step scale (`xs / sm / md / lg / xl`) plus a focus glow. We **never** use neon glows or colored shadows — except the focus ring (teal at 22% alpha) and the bottom-nav active pill (teal at 30% alpha).

### 4.6 Corner radii
Restrained. Most components live at **6–10px**. Cards `lg=10`. Modals `xl=14`. Pills (status chips, bottom nav, toggles) use `pill = 9999px`. Squares and right angles in tables — ledgers reward precision.

### 4.7 Transparency & blur
**Minimal use.** Modals dim the canvas with `rgba(14,23,41,0.32)` and a 4px backdrop blur, nothing more. We avoid glassmorphism on data surfaces because it reduces legibility — the brief is explicit: "trust & clarity over wow".

### 4.8 Motion
- Default duration `200ms` with `ease-out` for entrance/exit.
- Hover/focus snaps in `120ms` with `ease-soft`.
- Loading states (AI chat thinking, query running) — gentle 1.4s opacity pulse, never bouncing/scaling.
- The bottom nav's active pill slides between tabs with `300ms ease-in-out` and a soft glow. This is the only "signature" motion.
- **No spring bounces.** No `ease-spring`. The brand is calm; springs feel toy-like.

### 4.9 Hover / press states
- **Hover** — primary buttons darken to `--brand-primary-hover` (manually mixed, not just `filter: brightness`). Secondary buttons gain `var(--bg-subtle)`. Ghost buttons gain `var(--brand-primary-soft)`. No translation, no scale.
- **Pressed** — primary buttons drop one more step to `--brand-primary-pressed`. Scale `0.98` allowed but optional.
- **Focus (keyboard)** — `var(--shadow-focus)` halo, always visible. We never `outline: none` without replacement.
- **Disabled** — `opacity: 0.55`, no hover state, cursor: not-allowed.

### 4.10 Imagery tone
This is an internal data tool — there is almost no imagery. When imagery is unavoidable (about screen, marketing site), prefer:
- Warm, paper-tone, slightly muted color treatment
- B&W or duo-tone in teal+ink
- Never cool blue tech stock photos
- Never AI-generated mockup-product photography

### 4.11 Density & layout
- **Desktop-native** density. Padding is generous in forms (16–20px), compact in tables (8–10px row height).
- Information hierarchy by **weight and size** first; color second; spacing third. We rarely use background blocks to group.
- Page max width 1280px. Tables stretch full width with horizontal scroll if needed.
- Fixed elements: bottom navigation pill (always visible, centered horizontally, 16px from window bottom).

### 4.12 Cards
Every card has:
- Background `--bg-elevated` (cleanest white)
- 1px `--border-subtle` border
- `--radius-lg` (10px) corners
- `--shadow-xs` (barely visible)
- 14–16px internal padding

We don't use colored-left-border accent cards. We don't use icon-in-corner cards. We don't use gradient-bg cards. Just neat ruled rectangles, with badges and numerics doing the visual work.

---

## 5 — Iconography

**Primary set:** [Lucide](https://lucide.dev/) — chosen for clean 1.8-weight strokes, generous coverage of business/data concepts, and CDN availability.

### Usage rules
- **Default size:** 22px in headers, 18px in inline contexts (toasts, badges, buttons), 16px in dense tables, 14px in micro labels.
- **Stroke width:** always 1.8 (Lucide default). Never bumped to 2.5+ — feels heavy against Plex Arabic.
- **Color:** `currentColor` — inherits from the text it sits beside. Active nav uses `--fg-on-brand`; inactive uses `--fg-2`.
- **Layout:** icons sit before label in LTR languages, but **in RTL we flip directional icons** (arrows, chevrons, send) so they point correctly. Non-directional icons (settings cog, bell, calendar) stay un-mirrored.

### Special: the AI icon
The Intelligence tab uses `sparkles` (★) rather than a robot — the brief explicitly rejects "robot cliché". The Mihbar logo's accent badge echoes this with a check-spark.

### Emoji
Not used. (See Content Fundamentals.)

### Custom assets in `/assets`
- `logo-mark.svg` — 64×64 monogram, square, the favicon and dock icon
- `logo-lockup.svg` — 240×64 mark + Arabic wordmark + Latin sublabel

### Loading via CDN
For prototypes and HTML artifacts we inline SVGs from Lucide. For production Tauri, we recommend the `lucide-react` package and tree-shake imports.

---

## 6 — Files in this design system

```
.
├── README.md                      ← you are here
├── SKILL.md                       ← agent-skill manifest (Claude Code compatible)
├── colors_and_type.css            ← all design tokens (CSS variables)
│
├── brief/
│   └── DESIGN_SYSTEM_BRIEF.md     ← original product/identity brief
│
├── assets/
│   ├── logo-mark.svg              ← square monogram
│   └── logo-lockup.svg            ← horizontal lockup
│
├── preview/                       ← Design System tab cards (registered)
│   ├── _card.css                  ← shared card chrome
│   ├── brand-*.html               ← logo, story, voice
│   ├── colors-*.html              ← brand, surfaces, ink, semantic, dark
│   ├── type-*.html                ← families, scale, weights, numerics
│   ├── spacing-*.html             ← scale, radii, shadows
│   ├── motion-easing.html
│   ├── comp-*.html                ← buttons, inputs, badges, cards,
│   │                                schedule, chat, nav, toasts, table, modal
│   └── iconography.html
│
└── ui_kits/
    └── altabi-desktop/            ← the desktop app UI kit
        ├── README.md
        ├── index.html             ← interactive click-thru
        ├── *.jsx                  ← individual component recreations
        └── ...
```

---

## 7 — Caveats & flags

- **Fonts loaded from Google Fonts CDN.** For an offline Tauri build, download `IBM Plex Sans Arabic`, `IBM Plex Mono`, and `Reem Kufi` woff2 files and switch the `@import` in `colors_and_type.css` to local `@font-face` declarations. We did not bundle font files yet.
- **Original codebase was not attached.** The system is built from the brief and proposes a new identity (Mihbar). The current code presumably has indigo/violet/burgundy/cosmic themes — those are not represented here.
- **Icons are inlined SVGs from Lucide.** For production, switch to `lucide-react`.
- **No real product screenshots exist yet** — the UI kit screens are recreated from the brief's screen list, not from a live app.

---

*Updated: 2026-05-28*
