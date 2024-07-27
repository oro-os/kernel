import os
from os import path
from ..log import log
import subprocess
from . import get_site_packages_dir

LIMINE_GIT_URL = "https://github.com/oro-os/limine.git"
LIMINE_REF = "v7.0.3-binary"


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
                return limine_dir
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

    return limine_dir
