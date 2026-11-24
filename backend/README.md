# Private PoC feedback service

This directory deploys independently of the Rust/WASM game on GitHub Pages. It
contains one TypeScript Cloudflare Worker, D1 migrations, and a private R2 binding.
The game runs with no API configured or available. There is no public read API,
admin dashboard, account system, leaderboard, or change to simulation rules.

## Local setup (no production credentials)

From the repository root:

```sh
npm ci
npm --prefix backend ci
cp .env.example .env.local
cp backend/.dev.vars.example backend/.dev.vars
npm --prefix backend run migrate:local
npm --prefix backend run dev
```

In a second terminal, build the game as described in the root README and run
`npm run dev`. Vite reads the public `PV_FEEDBACK_API_URL` and
`PV_TURNSTILE_SITE_KEY` values from `.env.local`; restart Vite after changing them.
The examples use Cloudflare's public Turnstile testing keys on loopback only.
Choose a random local `RATE_LIMIT_SALT` (32 or more characters). Do not copy the
local testing secret into production. Production mode rejects dummy keys.
The real local widget still needs access to Cloudflare; automated tests replace
both the widget script and Siteverify responses and require no Cloudflare login.

```sh
npm --prefix backend test
python scripts/feedback_smoke.py
```

Install the root `requirements-browser.txt` and Chromium first. The API tests run
actual D1 and R2 bindings in Miniflare/workerd, with only Siteverify mocked. The
browser test uses a loopback test harness (never deployed) and the built `dist/`
site under a simulated GitHub Pages project path. It checks screenshot contents,
explicit consent, metadata restrictions, uploads, and lost-response retries.

## Production setup checklist

1. Enable Workers, D1, R2, and Turnstile in your Cloudflare account. Review the
   account's storage/request/CPU limits and billing before enabling public writes.
2. Log in interactively from `backend/`:

   ```sh
   npx wrangler login
   npx wrangler whoami
   npx wrangler d1 create phase-vector-feedback
   npx wrangler r2 bucket create phase-vector-feedback
   ```

   `backend/wrangler.jsonc` records the current owner's production D1 ID. When
   deploying to a different account, replace **only** the production database
   name and ID with your resource. This ID is not a secret and can be committed.
   Migration scripts use the `DB` binding, so local and production database names
   can differ. The local placeholder deliberately remains local.
   Leave R2 public access disabled: no r2.dev URL or public custom domain.
3. Create a **Managed Turnstile widget** for `kirisaki.github.io`. Its site key is
   public; its secret key belongs only in the Worker. If the game origin changes,
   update the widget hostname, `ALLOWED_ORIGINS`, and `TURNSTILE_HOSTNAMES` together.
   Origins contain scheme and host, **not** `/phase-vector/`. Do not allow `*`.
4. Store secrets interactively; do not put values in source, GitHub variables,
   shell command arguments, feedback, or chat:

   ```sh
   npx wrangler secret put TURNSTILE_SECRET_KEY --env production
   npx wrangler secret put RATE_LIMIT_SALT --env production
   ```

   Use a newly generated random value of at least 32 characters for the salt.
   If Wrangler requests creation of the Worker before storing its secrets, allow
   that creation or deploy once first; missing secrets fail closed with HTTP 503.
5. Keep `SCREENSHOT_RETENTION_DAYS` at 30 or choose 1–365 days. Configure R2's
   lifecycle with the **same** number of days as an independent orphan-cleanup
   fallback:

   ```sh
   npx wrangler r2 bucket lifecycle add phase-vector-feedback screenshot-retention screenshots/ --expire-days 30
   npm run migrate:production
   npm run deploy
   ```

   The hourly scheduled Worker removes expired images and stale pending writes,
   up to 20 records per run, and clears expired rate-limit records. R2 lifecycle
   deletion is asynchronous, not an exact-to-the-second guarantee. Monitor cleanup
   errors/backlogs; changing the Worker setting affects newly uploaded images.
   Set a lifecycle policy before accepting public uploads.
6. In GitHub repository **Settings → Secrets and variables → Actions → Variables**,
   add `FEEDBACK_API_URL` (the HTTPS Worker origin, no path) and
   `TURNSTILE_SITE_KEY` (the public key). Run the existing **CI and Pages** workflow
   to rebuild the game with these public settings. No Cloudflare API token is
   needed by the Pages build. Blank variables leave delivery disabled.
7. On the real Pages site, explicitly send a small test report and verify its D1
   row and private image. Test Turnstile with the real hostname, expiration, and a
   mobile browser. This is the production check that cannot be replaced by mocks.

### Optional GitHub backend deployment

The **Feedback backend** workflow runs tests on backend changes without secrets.
For its independent, manual deployment job, configure the `cloudflare-production`
GitHub environment with secret `CLOUDFLARE_API_TOKEN` and variable
`CLOUDFLARE_ACCOUNT_ID`. Limit the token to the intended account with Workers
Scripts edit, D1 edit, and Workers R2 storage edit permissions needed by Wrangler.
Review permissions against your account policy; use environment protection if
required. Then dispatch this workflow on `main` with `deploy=true`. It applies
migrations before deploying. Runtime Turnstile/salt secrets are set separately
with Wrangler and are never stored in the frontend. Deployment is not coupled to
Pages pushes. Local `npm run deploy` remains an alternative to GitHub automation.

## API contract

- `POST /v1/feedback`: anonymous explicit feedback, rating 1–5, category
  `bug | controls | balance | performance | other`, message 1–4,000 characters,
  metadata, and optional PNG.
- `POST /v1/results`: optional explicit terminal-result reports, using the same
  metadata and protections but no rating, category, message, or image. The game
  does **not** automatically transmit play results; this endpoint is available for
  a future opt-in caller. Use Turnstile action `result` instead of `feedback`.
- `GET /health`: process liveness only; it does not verify secrets/storage.
- No report listing, screenshot download, or administrative HTTP endpoint exists.

JSON requests use `{ "payload": { ... }, "token": "..." }`. Image uploads use
multipart fields `payload` (JSON string), `token`, and optionally `screenshot`.
`payload` has UUIDv4 `sessionId` and `submissionId`, the feedback fields for
`/v1/feedback`, and this metadata allowlist:

```json
{
  "version": "git-commit",
  "outcome": "running",
  "runKind": "standard",
  "simulationSeconds": 120,
  "forecastSeconds": 240,
  "score": null,
  "shotsFired": 4,
  "ownDeltaVSpent": 1000,
  "viewport": { "width": 1360, "height": 1000 }
}
```

Times are seconds and own delta-v uses m/s. `forecastSeconds` is the selected
absolute firing/preview time, or null for Live. Scores are null before a terminal
mission result (and in the sandbox). No enemy state, observations, detailed score
object, Lua content, full page URL, user agent string, or browsing history is
collected. All values remain untrusted client reports, not verified game results.
Unknown fields are rejected rather than silently retained.

The frontend creates a random ID per tab session using sessionStorage; it does
not fingerprint or link browsers across sessions. Closing the dialog discards
its screenshot; sending requires explicit consent and a separate attachment
checkbox. Turnstile loads only after consent and the Verify button. Its service
processes its own verification data. Free text and images can still contain
information voluntarily supplied by a user, so preview and review matter.

## Abuse controls and failure behavior

- Exact origin CORS plus server-side Siteverify success, hostname, and action
  validation. CORS alone is not authentication. Production never bypasses the
  challenge. Siteverify timeout/errors fail closed. Tokens are also claimed in
  D1 to prevent immediate replay during Siteverify's convergence window.
- Atomic D1 counters allow five attempts per session and twenty per IP in a
  ten-minute fixed window, shared across endpoints. Invalid/duplicate attempts
  also consume relevant limits. The database stores salted, window-specific IP
  hashes, not raw IP addresses. Cloudflare itself still processes connection data.
- Streamed body reads stop at 2 MiB + 24 KiB even without Content-Length. Images
  must be PNG, at most 2 MiB, at most 4,096 pixels per side and four million pixels.
  PNG signature, chunks, CRCs, 8-bit RGB/RGBA pixel dimensions, deflate size, and
  row filters are validated. Interlaced/indexed images and text/EXIF metadata are not
  accepted; the application's generated screenshots use the supported format.
- Stable session/submission IDs plus content fingerprints prevent duplicate
  writes. A completed identical retry returns 200 with the original ID. Changed
  content under the same ID returns 409. Fresh requests require fresh Turnstile
  tokens; a known completed retry can acknowledge success without redeeming a
  token again. The UI preserves its ID across uncertain network responses.
- D1 and R2 do not share a transaction. Pending rows claim the upload, storage
  errors trigger compensation, and the hourly job recovers abandoned writes
  after one hour. Concurrent in-progress requests return 409, never duplicate
  image objects. Generic errors do not log payloads, images, tokens, or stack
  traces. Worker observability logging is disabled by default.
- Structured reports remain in D1 until the operator deletes them. Only images
  and transient abuse-control records have automatic expiration in this PoC.
  No public analytics, automatic result upload, or game-event collection exists.

## Private inspection

Use your authenticated Cloudflare dashboard or Wrangler. For example, from
`backend/`:

```sh
npx wrangler d1 execute DB --remote --env production --command "SELECT id,kind,created_at,json_extract(payload_json,'$.category') AS category,image_key FROM submissions WHERE status='complete' ORDER BY created_at DESC LIMIT 20"
npx wrangler r2 object get phase-vector-feedback/screenshots/REPLACE_WITH_REPORT_ID.png --remote --file /tmp/feedback.png
```

Inspect message/metadata with a parameterized query or a selected row in the
console. Do not publish raw exports, add a public R2 domain, or commit downloaded
images. There is no application-level admin API to secure in this scope.

## Screenshot method and verification limits

The game canvas requests `preserveDrawingBuffer` when Bevy first creates its
WebGL context. This keeps the composited frame readable; an animation-frame
callback alone proved unreliable after browser compositing. This can increase
GPU memory/bandwidth use, so performance on physical mobile devices needs review.
`web/feedback-capture.ts` then copies the framebuffer within a render frame,
then supplies that frozen image while html-to-image clones the application DOM.
HTML controls and SVG tactical overlays are included; resolved SVG paint and text
styles are inlined on frozen SVG clones. Temporary serialization hooks are
restored afterward, without changing live SVG styles. The clone excludes the Lua
panel, local error text, final true-enemy-fuel detail, scripts, and iframes. It is
captured before opening the feedback dialog, stays on the device until explicitly
attached and sent, and is discarded when closed or removed. The capture dependency
is bundled only for this module; the rest of the UI remains plain TypeScript
modules. Its MIT license is shipped in `dist/licenses/html-to-image.txt`.

Capture uses the visible viewport at up to 1,600 pixels on its longest side,
without browser chrome or content outside the application viewport. Rendering,
canvas security, extension-injected content, font rasterization, or browser
limitations can prevent exact reproduction; failure leaves text submission
available. The preview is authoritative for what will be attached. Physical iOS,
Safari, Firefox, production Turnstile, and real Cloudflare deployment remain
separate checks from the automated Chromium/local workerd evidence.

References: [Turnstile server validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/),
[R2 lifecycles](https://developers.cloudflare.com/r2/buckets/object-lifecycles/),
[Workers testing](https://developers.cloudflare.com/workers/testing/), and
[html-to-image](https://github.com/bubkoo/html-to-image).
