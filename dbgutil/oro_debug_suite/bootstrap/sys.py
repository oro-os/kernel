import subprocess
from ..log import log, error


def check_bin_dep(name, *args, indent=0):
    """
    Checks to see if a singular binary dependency exists on the PATH.

    Returns True if the binary exists, False otherwise.
    """

    try:
        subprocess.run(
            [name, *args], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        log(f"{' ' * indent}found '{name}'")
    except subprocess.CalledProcessError as e:
        error(f"{' '* indent}could not find '{name}' in PATH: {e}")
        return False
    return True


def check_bin_deps(*args):
    """
    Checks to see if all required binary dependencies exist on the PATH.
    """

    check = lambda *args, **kwargs: check_bin_dep(*args, **kwargs, indent=4)

    ok = True

    for arg in args:
        ok = check(*(arg if type(arg) is list else [arg, "--version"])) and ok

    return ok
