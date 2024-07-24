import sys
import os
from os import path
import tempfile
import subprocess

import gdb

from .log import log
from .prompt import *

LIMINE_GIT_URL = "https://github.com/oro-os/limine.git"
LIMINE_REF = "v7.0.3-binary"


def print_logo():
    """
    Print the ORO OS logo.
    """

    log("")
    log("  ⠀⠀⠀⠀⠀⠀⠀⣀⣤⣤⣤⣤⣤⣀⠔⠂⠉⠉⠑⡄")
    log("  ⠀⠀⠀⠀⢠⣴⠟⠋⠉⠀⠀⠀⠉⠙⠻⣦⣀⣤⣤⣇")
    log("  ⠀⠀⠀⣰⡟⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⠟⠉⠉⢻⣧⠀    ORO OPERATING SYSTEM")
    log("  ⠀⠀⢰⡿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢿⣆⡀⢀⣸⡟⠀    kernel debug utilities")
    log("  ⠀⠀⢸⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠛⢻⡟⠋⠀⠀    github.com/oro-os/kernel")
    log("  ⠀⠀⠸⣷⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠊⠀⠀⣿⠃⠀⠀⠀    copyright (c) 2024, Josh Junon")
    log("  ⠀⠀⡐⠹⣧⡀⠀⠀⠀⠀⠀⡠⠊⠀⠀⢀⣾⠏⠀⠀⠀⠀    MPL-2.0 License")
    log("  ⠀⢰⠀⠀⠘⠻⣦⣄⣀⡔⠊⠀⣀⣠⣴⠟⠁")
    log("  ⠀⠘⢄⣀⣀⠠⠔⠉⠛⠛⠛⠛⠛⠉")
    log("")


def get_site_packages_dir():
    """
    Returns the site-packages directory for the debug suite.
    """

    site_dir = path.join(tempfile.gettempdir(), "oro_debug_suite_site_packages")
    if not path.exists(site_dir):
        os.mkdir(site_dir)

    return site_dir


def check_bin_dep(name, *args):
    """
    Checks to see if a singular binary dependency exists on the PATH.

    Returns True if the binary exists, False otherwise.
    """

    try:
        subprocess.run(
            [name, *args], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        log(f"    found '{name}'")
    except subprocess.CalledProcessError as e:
        log(f" !  could not find '{name}' in PATH: {e}")
        return False
    return True


def check_bin_deps():
    """
    Checks to see if all required binary dependencies exist on the PATH.
    """

    log("checking for required binaries")
    ok = True
    ok = ok and check_bin_dep("git", "--version")
    ok = ok and check_bin_dep("make", "--version")
    return ok


def install_deps():
    """
    Installs all pip dependencies for the debug suite.

    The dependencies are installed into a temporary site-packages directory.
    """

    log("python executable:", sys.executable)
    site_dir = get_site_packages_dir()
    log("using site-packages directory:", site_dir)
    sys.path.append(site_dir)

    requirements_txt = path.join(
        path.dirname(path.dirname(__file__)), "requirements.txt"
    )

    if not path.exists(requirements_txt):
        log("could not find requirements.txt; something is wrong with the debug suite")
        log("please report this issue to the developers")
        log("aborting suite bootstrap")
        return

    flag_file = path.join(site_dir, "deps_installed")

    if path.exists(flag_file) and path.getmtime(flag_file) > path.getmtime(
        requirements_txt
    ):
        # Skipping installation of dependencies
        return

    log("installing dependencies")

    pip_environ = os.environ.copy()
    pip_environ["PIP_TARGET"] = site_dir

    subprocess.run(
        [
            sys.executable,
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--no-python-version-warning",
            "-I",
            "--no-input",
            "-r",
            requirements_txt,
        ],
        check=True,
        env=pip_environ,
    )

    with open(flag_file, "w") as f:
        f.write("")

    log("dependencies installed")


def fetch_limine():
    """
    Fetches the limine bootloader from the Oro repositories
    and builds the limine utility.
    """

    limine_dir = path.join(get_site_packages_dir(), "limine")
    limine_flag = f"{limine_dir}.version"

    if path.exists(limine_flag):
        with open(limine_flag, "r") as f:
            if f.read().strip() == LIMINE_REF:
                log("limine at correct version; skipping fetch")
                return
            else:
                log("limine version mismatch; re-fetching")
                os.remove(limine_flag)
    else:
        log("fetching limine bootloader version", LIMINE_REF)

    if path.exists(limine_dir):
        import shutil

        log("removing existing limine directory")
        shutil.rmtree(limine_dir)

    subprocess.run(
        [
            "git",
            "clone",
            "--depth=1",
            "-c",
            "advice.detachedHead=false",
            "--branch",
            LIMINE_REF,
            LIMINE_GIT_URL,
            limine_dir,
        ],
        check=True,
    )

    log("limine fetched; building utility")

    subprocess.run(
        ["make", "-C", limine_dir, "limine"],
        check=True,
    )

    log("limine built")

    with open(limine_flag, "w") as f:
        f.write(LIMINE_REF)


def bootstrap_debug_suite():
    """
    Sets up all pre-requisites for the debug suite
    and registers all necessary hooks with GDB.
    """

    print_logo()

    if not check_bin_deps():
        log(
            "missing one or more required binaries on the PATH; aborting suite bootstrap"
        )
        log("please ensure that the stated binaries are available and restart GDB")
        return

    fetch_limine()
    install_deps()

    log("")
    log(
        "Oro kernel debug suite is ready; run \x1b[1mhelp oro\x1b[m for a list of commands"
    )


bootstrap_debug_suite()


class OroCmd(gdb.Command):
    """
    Oro operating system kernel debug suite GDB commands.
    """

    def __init__(self):
        super(OroCmd, self).__init__("oro", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro")


OroCmd()

from .cmd import *
