# Demo video

Records the presentation demo video from the real web client and API.

1. Start the stack and reset the demo data:

   ```bash
   docker compose up -d --build
   docker compose exec api python scripts/seed_demo.py --reset
   docker compose exec redis redis-cli flushall
   ```

2. Capture the security script output (it is replayed in the last scene):

   ```bash
   mkdir -p e2e/demo-video/out
   sh backend/scripts/demo_security.sh > e2e/demo-video/out/security-output.txt
   docker compose exec api python scripts/seed_demo.py --reset
   docker compose exec redis redis-cli flushall
   ```

3. Record and compose (needs Node 18+, `npm install playwright`,
   `npx playwright install chromium` and an ffmpeg with libx264):

   ```bash
   node e2e/demo-video/record.js
   node e2e/demo-video/compose.js     # -> e2e/demo-video/out/always-together-demo.mp4
   ```

The recording is two real browser sessions side by side: Maria (receives,
left) and Alex (shares, right). Captions are added per step from the
timestamps written by `record.js`. Recording on a machine with internet
access shows OpenStreetMap tiles on the map; the committed video in
`docs/assets/presentation/` was recorded where tiles were blocked, so its map
background is blank.

Options (environment variables): `BASE_URL`, `OUT_DIR`, `CHROMIUM_PATH`,
`FFMPEG`, `FONTS_DIR` (offline copy of the Google Fonts CSS and files),
`DEMO_PASSWORD`.
