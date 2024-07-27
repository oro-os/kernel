import subprocess
from ..log import log, error


def check_bin_dep(name, *args):
    """
    Checks to see if a singular binary dependency exists on the PATH.

    Returns True if the binary exists, False otherwise.
    """

    try:
        subprocess.run(
            [name, *args], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        log(f"found '{name}'")
    except subprocess.CalledProcessError as e:
        error(f"could not find '{name}' in PATH: {e}")
        return False
    return True


def check_bin_deps(*args):
    """
    Checks to see if all required binary dependencies exist on the PATH.

    Each argument can be a string or a list of strings, where the first element
    is the binary name and the rest are arguments to pass to it. If the argument
    is a string, it is treated as if it were `[arg, "--version"]`. To run just a
    program without any arguments, pass the program name as the only item in a list.
    """

    ok = True

    for arg in args:
        ok = check_bin_dep(*(arg if type(arg) is list else [arg, "--version"])) and ok

    return ok
