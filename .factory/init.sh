#!/usr/bin/env python3
"""No-op init for the DWG parser mission.

The current mission requires no environment bootstrap. Keeping this file as a
Python no-op makes it safe for worker environments that invoke `.factory/init.sh`
through Python on Windows.
"""

raise SystemExit(0)
