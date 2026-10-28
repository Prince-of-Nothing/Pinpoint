/*
 * Builds the presentation demo video from the recordings made by record.js.
 *
 *   sh backend/scripts/demo_security.sh > e2e/demo-video/out/security-output.txt
 *   node e2e/demo-video/compose.js          # -> out/always-together-demo.mp4
 *
 * Renders (with Playwright) a title card, one caption bar per step and an
 * animated terminal that replays the real security-script output, then
 * stitches everything with ffmpeg (needs libx264; set FFMPEG to its path).
 */
"use strict";
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");
const { chromium } = require(process.env.PW_PATH || "playwright");

const OUT = process.env.OUT_DIR || path.join(__dirname, "out");
const FFMPEG = process.env.FFMPEG || "ffmpeg";
const W = 1920;
const H = 1080;
const FPS = 30;
const TITLE_SECONDS = 4;

const CAPTIONS = {
  friends: "Maria and Alex are friends, but friendship alone shares nothing",
  share: "Alex shares with Maria: exact position, for 1 hour",
  live: "Maria sees Alex live, with accuracy, battery and freshness",
  move: "Alex moves, and Maria's map follows",
  checkins: "Check-ins: Jordan missed his; Alex taps “I'm OK”",
  stop: "Alex stops sharing, and Maria loses access immediately",
  attack: "Then we attack our own API: every attempt is refused",
};

// Google Fonts by default; FONTS_DIR may point to a local copy (fonts.css + font files) for offline use
const FONT_IMPORT = process.env.FONTS_DIR
  ? "fonts.css"
  : "https://fonts.googleapis.com/css2?family=DM+Serif+Display&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;600&display=block";
const FONTS = `
  @import url('${FONT_IMPORT}');
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html, body { width: ${W}px; height: ${H}px; }`;

function captionHtml(number, text) {
  return `<html><head><style>${FONTS}
    body { background: transparent; font-family: Inter, sans-serif; }
    .bar { position: absolute; left: 50%; bottom: 36px; transform: translateX(-50%); display: flex; align-items: center; gap: 22px;
           background: rgba(32,34,40,0.92); color: #fff; border-radius: 999px; padding: 18px 40px 18px 18px;
           box-shadow: 0 10px 30px rgba(0,0,0,.25); white-space: nowrap; }
    .n { width: 52px; height: 52px; border-radius: 50%; background: #6366F1; display: flex; align-items: center; justify-content: center;
         font-weight: 600; font-size: 26px; }
    .t { font-size: 32px; font-weight: 500; letter-spacing: .01em; }
  </style></head><body><div class="bar"><div class="n">${number}</div><div class="t">${text}</div></div></body></html>`;
}

const LABELS_HTML = `<html><head><style>${FONTS}
  body { background: transparent; font-family: Inter, sans-serif; }
  .l { position: absolute; top: 9px; transform: translateX(-50%); padding: 5px 16px; border-radius: 999px;
       background: rgba(99,102,241,.95); color: #fff; font-size: 19px; font-weight: 600; white-space: nowrap; }
  .d { position: absolute; left: 959px; top: 0; width: 2px; height: ${H}px; background: #D7D7E2; }
</style></head><body><div class="d"></div>
  <div class="l" style="left:${W / 4}px">Maria · receives</div>
  <div class="l" style="left:${(3 * W) / 4}px">Alex · shares</div></body></html>`;

const TITLE_HTML = `<html><head><style>${FONTS}
  body { background: #F4F4F6; font-family: Inter, sans-serif; color: #202228; display: flex; flex-direction: column;
         align-items: center; justify-content: center; }
  h1 { font-family: 'DM Serif Display', serif; font-weight: 400; font-size: 150px; line-height: 1.1; }
  .s { margin-top: 26px; font-size: 30px; letter-spacing: .14em; text-transform: uppercase; }
  .m { margin-top: 70px; font-size: 26px; color: #6B7280; }
</style></head><body><h1>Live demo</h1><div class="s">Always Together</div>
  <div class="m">Two real browser sessions against the running API · Maria receives, Alex shares</div></body></html>`;

function terminalHtml(lines) {
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const colour = (line) => {
    if (line.startsWith("$")) return `<span class="dim">${esc(line)}</span>`;
    if (line.startsWith("==")) return `<span class="h">${esc(line)}</span>`;
    return esc(line)
      .replace(/-&gt; (200)/g, '<span class="ok">-&gt; $1</span>')
      .replace(/-&gt; (40[0-9]|42[0-9])/g, '<span class="no">-&gt; $1</span>');
  };
  const html = lines.map((l) => `<div class="line">${colour(l) || "&nbsp;"}</div>`).join("");
  return `<html><head><style>${FONTS}
    body { background: #F4F4F6; display: flex; justify-content: center; padding-top: 34px; }
    .t { width: 1560px; height: 880px; background: #202228; border-radius: 22px; box-shadow: 0 16px 40px rgba(32,34,40,.3); overflow: hidden; }
    .bar { display: flex; gap: 11px; padding: 16px 24px; background: #2b2e36; }
    .bar i { width: 15px; height: 15px; border-radius: 50%; display: block; }
    .body { padding: 18px 34px 24px; font-family: 'JetBrains Mono', monospace; font-size: 20px; line-height: 1.3; color: #d7d9e0;
            font-variant-ligatures: none; }
    .line { white-space: pre; visibility: hidden; }
    .line.on { visibility: visible; }
    .h { color: #a5a8ff; font-weight: 600; } .ok { color: #6ee7a0; } .no { color: #ff8a8a; } .dim { color: #8b8f9c; }
  </style></head><body><div class="t"><div class="bar"><i style="background:#ff5f57"></i><i style="background:#febc2e"></i>
  <i style="background:#28c840"></i></div><div class="body">${html}</div></div></body></html>`;
}

function securityLines() {
  const raw = fs.readFileSync(path.join(OUT, "security-output.txt"), "utf8").split("\n");
  // Keep the attack sections (3-9) and drop blank separators so the output fits one screen
  const start = raw.findIndex((l) => l.startsWith("== 3."));
  const kept = [];
  let note = false;
  for (const line of raw.slice(start)) {
    if (line.startsWith("==")) note = false;
    if (line.trim().startsWith("(")) note = true; // explanatory notes (may span lines) are left out
    if (!note && line.trim() !== "") kept.push(line);
  }
  return ["$ sh backend/scripts/demo_security.sh", ...kept];
}

function ff(args) {
  execFileSync(FFMPEG, ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: "inherit" });
}

(async () => {
  const { files, marks } = JSON.parse(fs.readFileSync(path.join(OUT, "marks.json"), "utf8"));
  const end = marks.find((m) => m.id === "end").t;
  const steps = marks.filter((m) => m.id !== "end");

  const browser = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
  const page = await (await browser.newContext({ viewport: { width: W, height: H } })).newPage();
  if (process.env.FONTS_DIR) fs.cpSync(process.env.FONTS_DIR, OUT, { recursive: true });
  // Pages are loaded from files so relative font paths resolve
  const open = async (target, html, name) => {
    const file = path.join(OUT, name);
    fs.writeFileSync(file, html);
    await target.goto("file://" + file, { waitUntil: "networkidle" });
  };
  const png = async (html, file, transparent = true) => {
    await open(page, html, file.replace(".png", ".html"));
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: path.join(OUT, file), omitBackground: transparent });
  };
  await png(TITLE_HTML, "title.png", false);
  await png(LABELS_HTML, "labels.png");
  const captionFiles = [];
  for (const [i, step] of steps.entries()) {
    const file = `caption-${step.id}.png`;
    await png(captionHtml(i + 1, CAPTIONS[step.id]), file);
    captionFiles.push(file);
  }
  await png(captionHtml(steps.length + 1, CAPTIONS.attack), "caption-attack.png");

  // Terminal scene: recorded while the lines appear one by one
  const lines = securityLines();
  const termCtx = await browser.newContext({ viewport: { width: W, height: H }, recordVideo: { dir: OUT, size: { width: W, height: H } } });
  const term = await termCtx.newPage();
  await open(term, terminalHtml(lines), "terminal.html");
  await term.evaluate(() => document.fonts.ready);
  await term.waitForTimeout(800);
  const rows = await term.$$(".line");
  for (const [i, row] of rows.entries()) {
    await row.evaluate((el) => el.classList.add("on"));
    await term.waitForTimeout(lines[i].startsWith("==") ? 650 : lines[i].startsWith("$") ? 900 : 380);
  }
  await term.waitForTimeout(3500);
  const termVideo = term.video();
  await termCtx.close();
  await termVideo.saveAs(path.join(OUT, "terminal.webm"));
  await termVideo.delete();
  await browser.close();

  // 1. Title card
  ff(["-loop", "1", "-t", String(TITLE_SECONDS), "-i", path.join(OUT, "title.png"),
      "-vf", `fps=${FPS},format=yuv420p,fade=t=in:st=0:d=0.6,fade=t=out:st=${TITLE_SECONDS - 0.6}:d=0.6`,
      "-c:v", "libx264", "-preset", "medium", "-crf", "20", path.join(OUT, "part1.mp4")]);

  // 2. Side-by-side browsers with labels and timed captions
  const inputs = ["-ss", String(files.maria.offset), "-i", files.maria.file, "-ss", String(files.alex.offset), "-i", files.alex.file,
                  "-i", path.join(OUT, "labels.png"), ...captionFiles.flatMap((f) => ["-i", path.join(OUT, f)])];
  let graph = `[0:v][1:v]hstack=inputs=2,fps=${FPS}[v0];[v0][2:v]overlay=0:0[v1]`;
  let last = "v1";
  steps.forEach((step, i) => {
    const from = step.t + 0.4;
    const to = (steps[i + 1] ? steps[i + 1].t : end) - 0.2;
    graph += `;[${last}][${i + 3}:v]overlay=0:0:enable='between(t,${from.toFixed(2)},${to.toFixed(2)})'[c${i}]`;
    last = `c${i}`;
  });
  graph += `;[${last}]format=yuv420p,fade=t=in:st=0:d=0.5,fade=t=out:st=${(end - 0.6).toFixed(2)}:d=0.6[out]`;
  ff([...inputs, "-filter_complex", graph, "-map", "[out]", "-t", end.toFixed(2),
      "-c:v", "libx264", "-preset", "medium", "-crf", "20", path.join(OUT, "part2.mp4")]);

  // 3. Terminal replay with its caption
  ff(["-i", path.join(OUT, "terminal.webm"), "-i", path.join(OUT, "caption-attack.png"),
      "-filter_complex", `[0:v]fps=${FPS}[t];[t][1:v]overlay=0:0,format=yuv420p,fade=t=in:st=0:d=0.5[out]`,
      "-map", "[out]", "-c:v", "libx264", "-preset", "medium", "-crf", "20", path.join(OUT, "part3.mp4")]);

  // Concatenate (same codec settings, so stream copy is safe)
  const list = path.join(OUT, "parts.txt");
  fs.writeFileSync(list, ["part1.mp4", "part2.mp4", "part3.mp4"].map((p) => `file '${path.join(OUT, p)}'`).join("\n"));
  ff(["-f", "concat", "-safe", "0", "-i", list, "-c", "copy", "-movflags", "+faststart", path.join(OUT, "always-together-demo.mp4")]);
  console.log("Wrote", path.join(OUT, "always-together-demo.mp4"));
})().catch((e) => { console.error(e); process.exit(1); });
