"""Rebuild the whole Nexus brand kit, in order.  Run:  python3 brand/tools/build_all.py"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
for step in ["build_fonts.py", "build_marks.py", "build_logo.py", "build_patterns.py", "build_icons.py", "build_tokens.py", "build_guidelines.py"]:
    print(f"\n== {step}")
    subprocess.run([sys.executable, os.path.join(HERE, step)], check=True, cwd=HERE)
