#!/usr/bin/env bash
# Run the Rust API server and the Vite dev server together; Ctrl-C stops both.
# Ports default to 3001 (API) and 5173 (web); override with API_PORT / WEB_PORT.

set -e

export API_PORT="${API_PORT:-3001}"
export WEB_PORT="${WEB_PORT:-5173}"

trap 'kill 0' EXIT

echo "API on :$API_PORT, frontend on :$WEB_PORT (proxies /api)"
cargo run -- serve --port "$API_PORT" &
npm run dev --prefix web &

wait
