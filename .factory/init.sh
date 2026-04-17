#!/usr/bin/env python3
"""No-op init for the PLAN-4-18 DWG parser mission.

Validation is cargo-only on Windows and requires no extra bootstrap beyond the
existing workspace and local ACadSharp sample directory. Keeping this file as a
Python no-op makes it safe for worker environments that invoke `.factory/init.sh`
through Python.
"""

raise SystemExit(0)
