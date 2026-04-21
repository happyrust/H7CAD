#!/usr/bin/env sh
# PID real-sample display/screenshot mission bootstrap.
# Idempotent: ensure Cargo deps are fetchable; no long-running services.
cargo fetch --locked >/dev/null 2>&1 || true
exit 0
