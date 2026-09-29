"""Write the as-built OpenAPI schema to docs/reference/openapi.json.

    cd backend && python scripts/export_openapi.py
"""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
os.environ.setdefault("DATABASE_URL", "sqlite://")

from app.main import app  # noqa: E402

TARGET = Path(__file__).resolve().parents[2] / "docs" / "reference" / "openapi.json"

if __name__ == "__main__":
    TARGET.write_text(json.dumps(app.openapi(), indent=2, sort_keys=True) + "\n")
    print(f"Wrote {TARGET}")
