import os
from os import path
import subprocess
import tempfile
import sys
from ..log import log


def get_site_packages_dir():
    """
    Returns the site-packages directory for the debug suite.
    """

    site_dir = path.join(tempfile.gettempdir(), "oro_debug_suite_site_packages")
    if not path.exists(site_dir):
        os.mkdir(site_dir)

    return site_dir


def install_python_deps():
    """
    Installs all pip dependencies for the debug suite.

    The dependencies are installed into a temporary site-packages directory.
    """

    log("python executable:", sys.executable)
    site_dir = get_site_packages_dir()
    log("using site-packages directory:", site_dir)
    sys.path.append(site_dir)

    requirements_txt = path.join(path.dirname(__file__), "..", "..", "requirements.txt")

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
            "--upgrade",
            "-r",
            requirements_txt,
        ],
        check=True,
        env=pip_environ,
    )

    with open(flag_file, "w") as f:
        f.write("")

    log("dependencies installed")
