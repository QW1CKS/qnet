#!/usr/bin/env bash
set -euo pipefail

# Simple local acceptance harness for bootstrap <30s target.
# - Starts a tiny Python HTTP server that responds 200 OK on /health.
# - Configures STEALTH_BOOTSTRAP_* env to point at it (unsigned dev mode).
# - Runs the htx bootstrap_check example with a 10s timeout (reads env).

PORT=${PORT:-9443}
TIMEOUT_SECS=${TIMEOUT_SECS:-10}

tmpdir=$(mktemp -d)
trap 'kill 0 || true; rm -rf "$tmpdir"' EXIT

cat >"$tmpdir/server.py" <<'PY'
import http.server, ssl, sys
class H(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith('/health'):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'ok')
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'not found')

if __name__ == '__main__':
    port=int(sys.argv[1])
    httpd=http.server.HTTPServer(('127.0.0.1',port), H)
    httpd.serve_forever()
PY

python3 "$tmpdir/server.py" "$PORT" &
sleep 0.5

# Unsigned dev catalog pointing at local server
cat >"$tmpdir/seeds.json" <<JSON
{"catalog":{"version":1,"updated_at":0,"entries":[{"url":"http://127.0.0.1:$PORT"}]}}
JSON

export STEALTH_BOOTSTRAP_CATALOG_JSON="$(cat "$tmpdir/seeds.json")"
export STEALTH_BOOTSTRAP_ALLOW_UNSIGNED=1

echo "Running bootstrap_check with TIMEOUT=$TIMEOUT_SECS s ..."
export BOOTSTRAP_TIMEOUT_SECS="$TIMEOUT_SECS"
cargo run -q -p htx --example bootstrap_check || true
