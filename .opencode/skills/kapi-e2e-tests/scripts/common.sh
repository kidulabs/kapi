#!/bin/bash
# Common setup for all test scripts
# Source this file at the start of each test script

set -e pipefail 2>/dev/null || true

export TEST_RUN=${TEST_RUN:-$(date +%s)}

# Shared helpers
register_widget_schema() {
  curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
    -H "Content-Type: application/json" \
    -d "{\"targetGroup\":\"example.io.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"Widget\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"color\":{\"type\":\"string\"},\"size\":{\"type\":\"integer\"}},\"required\":[\"color\",\"size\"]}}" > /dev/null
}

register_widget_schema_with_status() {
  curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
    -H "Content-Type: application/json" \
    -d "{\"targetGroup\":\"example.io.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"Widget\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"color\":{\"type\":\"string\"},\"size\":{\"type\":\"integer\"}},\"required\":[\"color\",\"size\"]},\"statusSchema\":{\"type\":\"object\",\"properties\":{\"phase\":{\"type\":\"string\"},\"message\":{\"type\":\"string\"}}}}" > /dev/null
}

start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget${query}" > "$logfile" 2>&1 &
  echo $!
}

get_system_fields() {
  local name="$1"
  local body=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${name}")
  GET_RV=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
  GET_CREATED=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
  GET_UPDATED=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")
}

# Check server is running
check_server() {
  if ! lsof -ti :8080 > /dev/null 2>&1; then
    echo "ERROR: Server not running on port 8080"
    echo "Please start the server first:"
    echo "  RUST_LOG=kapi=trace cargo run > /tmp/kapi-server.log 2>&1 &"
    exit 1
  fi
}

check_server
