"""
Oro kernel debug util loader stub.

This python script will attempt to configure the paths to the
Oro kernel debug suite and load them when GDB loads.
"""

import gdb  # type: ignore
import sys
from os import path, getenv


def log(*args, **kwargs):
    print("\x1b[38;5;129moro\x1b[m", *args, **kwargs)


suite_loaded = False


def attempt_load_suite(suite_path, from_tty=False):
    global suite_loaded

    if suite_loaded:
        log("suite already loaded; restart GDB to specify a different location")
        return False

    suite_path = path.abspath(suite_path)
    init_path = path.join(suite_path, "oro_debug_suite", "__init__.py")

    if not path.exists(init_path):
        if from_tty:
            log(f"could not find Oro suite at {suite_path}")
        return False

    log(f"loading Oro suite from '{suite_path}'")

    sys.path.append(suite_path)

    try:
        import oro_debug_suite  # type: ignore
    except ImportError as e:
        log(f"failed to load Oro suite: {e}")
        return False

    suite_loaded = True
    return True


def bootstrap_debug_suite():
    log("welcome to the Oro kernel")
    log("resolving debug utilities...")

    env_suite_path = getenv("ORO_DBGUTIL")
    if env_suite_path:
        if attempt_load_suite(env_suite_path):
            return
        else:
            log(f"ORO_DBGUTIL points to an invalid path: {env_suite_path}")

    prog = gdb.current_progspace()
    current_filename = prog.filename
    if not current_filename:
        log(
            "somehow, the current program has no filename; set `ORO_DBGUTIL` to dbgutil path to manually load the Oro suite"
        )
        return

    current = current_filename
    while True:
        parent = path.dirname(current)
        if parent == current:
            log(
                "failed to find Oro suite (hit root); set `ORO_DBGUTIL` to dbgutil path to manually load the Oro suite"
            )
            return

        current = parent

        if attempt_load_suite(path.join(current)):
            return
        if attempt_load_suite(path.join(current, "dbgutil")):
            return


bootstrap_debug_suite()
