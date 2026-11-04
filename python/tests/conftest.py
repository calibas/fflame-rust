# The package from this checkout, not an installed one: tests run against
# the module `python/build.py` built (or wasm/python's build output).
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
