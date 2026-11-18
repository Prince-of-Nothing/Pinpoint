# Deck QA - Midterm 2 slides

Scope: `docs/midterm2/slides.html` and `docs/midterm2/Pinpoint-Midterm2.pdf`
Branch: `docs/midterm2` (report only; nothing committed)
Method: headless Chromium via Puppeteer, viewport 1600x900, DSF 1, all 13
sections forced active in turn; print pass via `page.pdf({preferCSSPageSize:true,
printBackground:true})`; PDF checked page-by-page with PyMuPDF at 100 dpi and
full-size referenced PNGs reviewed directly.

## Verdict summary

- 13/13 slides render; no element leaves the 1600x900 slide box; no clipped text.
- 6/6 `<img>` references resolve (`naturalWidth > 0`), 0 failed requests.
- No email address appears in slide text. No test email/account is visible in any
  referenced screenshot.
- Contrast: lowest is 5.71:1 (red note on slide 3); all body/caption/accent text is
  7.63-15.47:1. WCAG AA normal-text pass.
- PDF: 13 pages, A4 landscape, every page has text and ink; no blank, broken or
  missing-image page.
- Open fixes: 1 global (on-screen slide numbers are covered by the nav buttons) and
  4 slide-level (slide 3 copy, slide 7 legibility, slide 8 cropped source image,
  slide 11 letterboxing).

## Per-slide results

`G1` = the global slide-number/navigation collision described below. It affects
the on-screen number on every slide but is not repeated as a separate failure in
each row.

| # | Slide | Text / overflow | Images (aspect, legibility) | Contrast | Number | Verdict |
|---|-------|-----------------|-----------------------------|----------|--------|---------|
| 1 | Title (`Pinpoint`) | Fits; hero title/sub/meta clear; no overflow | n/a | 8.41-15.47:1 | FIX G1 | PASS |
| 2 | The problem | 3 cards fit at ~473px each; all copy wraps cleanly | n/a | 7.63-15.47:1 | FIX G1 | PASS |
| 3 | The rule | Arrow + 3 cards fit; no clipping | n/a | red note 5.71:1; rest 7.63+ | FIX G1 | FIX (copy/visual mismatch) |
| 4 | Delivered | 6 cards in 3x2 fit; no clipping | n/a | 7.63-15.47:1 | FIX G1 | PASS |
| 5 | Architecture | Fits at 1600; print reflows the Web card to two lines but still fits | n/a | 7.63-15.47:1 | FIX G1 | PASS |
| 6 | Security + privacy | 2x5 checklist fits; caption fits | n/a | 7.63-15.47:1 | FIX G1 | PASS |
| 7 | Offline | Captions fit; no clipping | 1440x1000 shown at 696x484 (0.48x); UI labels render ~5-7px on screen and ~5px in the PDF. Aspect preserved but evidence text is not legible | 8.41-15.47:1 | FIX G1 | FIX (legibility) |
| 8 | Today + private notes | Captions fit; no CSS clipping | Left 694x564 shown 664x540 (0.96x), sharp. Right 626x842 shown 402x540 (0.64x), aspect correct, but the source PNG itself is cut at the top and left: "Cafe" touches the edge and the first body character is clipped | 8.41-15.47:1 | FIX G1 | FIX (cropped asset) |
| 9 | Native clients | Captions fit; no clipping | 1500x1000 shown 835x557 (0.56x); chrome labels readable, secondary body text small but non-essential; metrics at right are clear | 7.63-15.47:1 | FIX G1 | PASS (minor legibility note) |
| 10 | Verification | 4 metrics + 6-row table fit; no clipping | n/a | 7.63-15.47:1 | FIX G1 | PASS |
| 11 | Live demo | Ordered list fits; URL text fits | Image box 731x558, source 1055x900 (1.172). `object-fit: contain` leaves ~38px empty bars on both sides inside the border; image itself is not distorted | 8.41-15.47:1 | FIX G1 | FIX (letterbox) |
| 12 | Limitations + next | Two 6-item columns fit; no clipping | n/a | 8.41-15.47:1 | FIX G1 | PASS |
| 13 | Thank you | Close copy and URL fit; 2-line subtitle at 1600 (1 line in PDF) | n/a | 8.41-15.47:1 | FIX G1 | PASS |

## Global findings

### G1. Nav covers every slide number (FIX)

Measured at 1600x900:

- `.num`: x 1469-1504, y 863-878.
- `.nav`: x 1430-1581, y 844-884.
- The next button (x 1476-1516) covers the number from 1476 onward, so only the
  first 7px of "1 / 13" is visible; the "/ 13" is always hidden. Nav z-index 5 wins.

Smallest fix: move the nav above the footer line.

`docs/midterm2/slides.html:112`

```css
.nav {
  position: fixed;
  right: 1.2rem;
  top: 1rem;
  bottom: auto;
  display: flex;
  gap: 0.4rem;
  z-index: 5;
}
```

Verified alternative if the nav must stay bottom-right: `bottom: 3.4rem` instead of
`1rem` clears the number at 900px height, but it sits only 4px below the enlarged
slide-11 image. Top-right is cleaner.

### G2. Print CSS (PASS)

`docs/midterm2/slides.html:115-121` already:

- sets `@page { size: A4 landscape; margin: 0; }`;
- forces `.slide { display:flex; height:100vh; page-break-after:always; }`;
- hides `.nav`;
- preserves dark backgrounds via `print-color-adjust: exact`.

Generated PDF: 13 pages, no extra blank from the final `page-break-after: always`.
Slide 5 is the tightest reflow (Web card wraps to two lines) and still ends well
inside the 793px print height. No print fix required.

Optional hardening only, not needed with the current Chrome:

```css
@media print {
  .slide:last-child { break-after: auto; }
}
```

### G3. Title slide (PASS, optional semantics)

Visually correct: date 18 November 2026, version 2.6.1, stack and platforms
present; the radial hero and 8vw brand fit at 1600x900 and in the PDF. Only
accessibility nit: the title is `<div class="brand">Pinpoint</div>`, so the deck
has no `h1`. Optional one-tag change with no visual effect:

```html
<!-- docs/midterm2/slides.html:128 -->
<h1 class="brand">Pinpoint</h1>
```

`.hero .brand` already overrides `h1` font sizing, so no CSS change is needed.
The title slide has no presenter name/affiliation; add it only if the rubric
requires identity.

### G4. Images and privacy (PASS)

- All six referenced assets exist and load: `07-offline.png`, `08-route.png`,
  `03-today-card.png`, `04-note-card.png`, `06-native.png`, `map-clean.png`.
- No request failures, no 404, no external script or font requests.
- Slide text contains no `user@host` pattern.
- Referenced screenshots show no email and no named test account. Slide 9 shows
  "No friends yet"; slide 8 shows generic coordinates and a timestamp only.
- Not referenced, but do not reuse without redaction: `assets/05-mobile.png`
  displays the anonymous handle `@lively-forest-5583`.

## Exact slide fixes

### F1. Slide 3: copy says "crossed arrow", but the arrow is a normal arrow

`docs/midterm2/slides.html:160`

```html
<p class="noarrow">Friendship alone shows nothing. No arrow is the default.</p>
```

This is the smallest correct fix. If the crossed arrow is desired instead, draw it
as a second arrow with a danger-colour slash and keep the original copy; that needs
new CSS and is not necessary for the message.

### F2. Slide 7: screenshot evidence is too small to read

`docs/midterm2/slides.html:235-236`

Two 1440px desktop captures in 696px columns cannot carry readable UI text. The
cheapest content fix is to surface the key evidence as real text under the right
image:

```html
<div class="caption">
  Native map server routes on the installed graph; web routing is online (OSRM).
  <b>134.4 km · 1 h 38 min · Auto.</b>
</div>
```

This adds one wrapped caption line at 1600 and in print; content still ends above
the footer. If the full route panel must be readable, replace the right-hand
full-map shot with a 1:1 crop of the left routing panel (`assets/08-route.png`,
x 0-390, y 270-1000) at roughly 300-360px wide. No CSS change is needed for the
caption option.

### F3. Slide 8: `04-note-card.png` is clipped in the source image

`docs/midterm2/slides.html:251` points at a PNG that is already cut: the "Cafe"
heading and the first body character touch the left/top edge, the card's top
rounded border is missing, and the orange accent line is cut at the right. CSS
cannot restore missing pixels.

Fix the asset, not the layout: re-export `assets/04-note-card.png` from the app at
2x DPR with the complete note card plus 16-24px of surrounding panel background on
all four sides. Keep the filename so the HTML stays unchanged. `.card-shot`
(`max-height: 60vh`) will still fit the new export. The left `03-today-card.png`
is correctly framed and sharp (0.96x) and needs no change.

### F4. Slide 11: map is letterboxed inside its border

Measured: image box 731x558 but source aspect 1.172 means the picture content is
654x558, leaving 38px of panel background on each side inside the border. The
image is not distorted, but the empty bars are visible on screen and in the PDF.

`docs/midterm2/slides.html:315`

```html
<img class="shot fit-width" src="assets/map-clean.png"
     alt="Pinpoint map with search and OSM attribution" />
```

Add after `docs/midterm2/slides.html:94`:

```css
.shot.fit-width { width: 100%; height: auto; max-height: none; }
```

Verified at 1600x900: element becomes 731x624, top 178, bottom 802; all map edges
and the search bar are preserved and the bars disappear. It still fits the slide
and reflows safely in print. Do not use `object-fit: cover` here: it crops the
search bar and the bottom attribution line.

## PDF checks

| Page | Content | Text chars | Ink | Images | Result |
|------|---------|-----------:|-----|--------|--------|
| 1 | Title | 225 | 1.36% | 0 | PASS |
| 2 | Problem | 375 | 2.35% | 0 | PASS |
| 3 | Rule | 418 | 2.76% | 0 | PASS |
| 4 | Delivered | 573 | 3.17% | 0 | PASS |
| 5 | Architecture | 593 | 3.65% | 0 | PASS |
| 6 | Security | 611 | 1.91% | 0 | PASS |
| 7 | Offline | 224 | 7.13% | 2 | PASS (small text) |
| 8 | Today + notes | 233 | 13.44% | 2 | PASS (clipped source) |
| 9 | Native | 338 | 12.16% | 1 | PASS |
| 10 | Verification | 558 | 3.80% | 0 | PASS |
| 11 | Live demo | 463 | 25.83% | 1 | PASS (letterbox) |
| 12 | Limitations | 558 | 2.00% | 0 | PASS |
| 13 | Thank you | 204 | 2.52% | 0 | PASS |

- Page count 13, media box 841.9x595.9pt (A4 landscape). `/Count 13`, 13 page objects.
- No page is blank: minimum 204 text characters and 1.36% non-background ink.
- All four image pages embed and render their images; no broken-image placeholder.
- The PDF carries the same slide 8 crop and slide 11 letterbox as the HTML, so
  regenerating after F3/F4 fixes both.
- Print-only reflow checked: slide 5 "ES modules · MapLibre · PMTiles" wraps to two
  lines and the PostgreSQL card still sits above the footer; no clipping.

## Commands used

- Puppeteer: `file:///C:/Users/Dinusauuuur/Desktop/git/Pinpoint/docs/midterm2/slides.html`
  at 1600x900, one screenshot per active slide.
- Print: `page.pdf({printBackground:true, preferCSSPageSize:true})` -> 13 pages.
- PDF inspection: PyMuPDF page count, page size, text length, image count and ink
  sampling at 100 dpi.

No files were changed except this report; nothing was committed.

