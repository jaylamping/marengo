"""Where the left arm CAD exports live. Change paths here only. See cad/README.md for the layout."""
from pathlib import Path

CAD = Path(r"J:\code\marengo\cad")
PARTS = CAD / "parts" / "left_arm"
FOREARM = CAD / "exports" / "forearm"
UPPER_ARM = CAD / "exports" / "upper-arm"
ARM = CAD / "exports" / "arm"
RENDER = CAD / "exports" / "left_arm" / "render"
