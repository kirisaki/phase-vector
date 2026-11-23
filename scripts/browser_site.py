"""Serve the release artifact to Playwright at the GitHub Pages project subpath."""

import mimetypes
from pathlib import Path
from urllib.parse import unquote, urlparse

RELEASE_URL = 'https://phase-vector.test/phase-vector/'
DIST = Path(__file__).resolve().parents[1] / 'dist'


def route_release_site(page):
    def serve(route):
        path = unquote(urlparse(route.request.url).path)
        prefix = '/phase-vector/'
        if not path.startswith(prefix):
            route.fulfill(status=404, body='Wrong project base path')
            return
        file = (DIST / (path[len(prefix):] or 'index.html')).resolve()
        if not file.is_relative_to(DIST) or not file.is_file():
            route.fulfill(status=404, body='Not found')
            return
        mime = 'application/wasm' if file.suffix == '.wasm' else mimetypes.guess_type(file)[0]
        route.fulfill(path=str(file), content_type=mime or 'application/octet-stream')

    # Real browser requests use the release artifact without a local listening port.
    page.route('https://phase-vector.test/**', serve)
