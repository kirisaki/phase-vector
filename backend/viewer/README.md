# Private read-only feedback viewer

A separate Worker renders a small HTML interface from the existing D1 database
and proxies screenshots from the existing private R2 bucket. It has no frontend
framework, JavaScript bundle, public asset bucket, write endpoint, or migrations.
It does not change the public ingestion Worker or the GitHub Pages game.

The Japanese interface lists completed feedback newest first, 25 per page, with
category, UTC timestamp, message excerpt, version, and attachment status. Filters
support category, the last 7/30/90 days, and an exact version. Desktop shows list
and detail together; phones open a detail screen with a back link. Result-only
submissions and incomplete uploads are excluded. Metadata is client-reported,
not verified gameplay evidence. Times use seconds and delta-v uses m/s.

## Local development and checks

From the repository root:

```sh
npm --prefix backend ci
npm --prefix backend run migrate:local
npm --prefix backend run viewer:dev
```

Open `http://127.0.0.1:8788`. The local configuration simulates Access as
`viewer@example.test`; it uses local D1/R2 storage, never production data. The
explicit persistence path shares storage with the local ingestion Worker when
both commands are run from the repository root as shown. An empty local database
shows an empty list; submit through the local game to populate it.

```sh
npm --prefix backend test
python3 -m pip install -r requirements-browser.txt
python3 -m playwright install chromium
python3 scripts/viewer_smoke.py
```

The tests seed isolated D1/R2 fixtures and simulate the runtime Access context,
including missing identity, wrong email, forged headers, and missing settings.
They cover read-only methods, filters, tied-timestamp pagination, escaped HTML,
image expiry before physical cleanup, missing images, and generic storage errors.
Chromium checks desktop and 390/320 px phone layouts, navigation, filters, and image
decoding. No Cloudflare credentials or production reports are needed. Physical
phones, Safari, Firefox, and the live email login flow are separate manual checks.

## Production deployment

1. From `backend/`, deploy the separate viewer:

   ```sh
   npm run viewer:deploy
   npx wrangler secret put VIEWER_EMAILS --config viewer/wrangler.jsonc --env production
   ```

   Enter the permitted full email address (or a comma-separated allowlist) at the
   secret prompt. Do not put it in public source or the frontend. The initial
   deployment returns 503 until this setting exists. Without authenticated Access,
   it returns 403 even after the allowlist is configured.
2. Enable Cloudflare Zero Trust if needed. In **Workers & Pages**, open only
   **phase-vector-feedback-viewer → Access → Protect this Worker behind Access**.
   Select **All traffic** (production and previews). Choose an account-member
   policy initially if necessary, then edit the corresponding application in
   **Zero Trust → Access → Applications** to allow **Emails**, using exactly the
   same email address as `VIEWER_EMAILS`. Use a verified email identity provider
   or One-time PIN. Do not allow a whole public email domain or Everyone.
   Do not enable account-wide protection: the anonymous ingestion Worker must
   remain public. Do not add a Bypass policy to the viewer.
3. Visit the viewer's deployed URL. Verify that a signed-out browser requires
   login; the allowed email can read a report and image; a different email cannot.
   Verify `/image/<report-id>` independently in a signed-out browser. Log out with
   the header link. Do not treat local tests as proof that a production policy
   has been configured correctly.

Deployment is independent of the API and Pages pipelines. The existing backend
workflow tests both Workers; its optional deployment job still deploys only the
ingestion API. Deploy viewer changes with `npm run viewer:deploy`.

## Security and limits

The Worker uses the runtime-authenticated `ctx.access.getIdentity()` and checks
the email allowlist on every page and image request. It never trusts identity
headers directly. Missing Access or missing identity fails closed. There is no
production debug-login switch. Wrangler's `access.dev` is local simulation only.
There is deliberately no Static Assets binding: that router does not propagate
`ctx.access`. WARP-only and service-binding calls without an email identity are
denied. See [Cloudflare's Access integration documentation](https://developers.cloudflare.com/workers/configuration/cloudflare-access/).

All responses use private/no-store caching, no-referrer, nosniff, and noindex.
HTML is escaped and has a restrictive nonce-based CSP with no scripts or frames.
SQL parameters are bound, list size is bounded, and pagination uses stable time
and UUID cursors. Filters may scan D1 JSON at PoC scale; add measured indexes only
if the report volume requires them. Images use authenticated same-origin routes,
never public R2 URLs. Expired images return 410 even before lifecycle deletion.
The public API's existing retention policy and cleanup remain authoritative.

The application performs SELECT and R2 get operations only. D1/R2 bindings are
not infrastructure-level read-only credentials, so review future Worker changes
accordingly. The viewer has no edit/delete/export/search dashboard features.
It does not retain new identity logs or send report data to third-party assets.
