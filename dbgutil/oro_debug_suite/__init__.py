import gdb  # type: ignore

from .log import log
from .bootstrap import install_python_deps


def print_logo_banner():
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


class OroCmd(gdb.Command):
    """
    Oro operating system kernel debug suite GDB commands.
    """

    def __init__(self):
        super(OroCmd, self).__init__("oro", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro")


OroCmd()
print_logo_banner()
install_python_deps()
from . import service as _
from . import cmd as _
from . import prompt as _

log("")
log("Oro kernel debug suite is ready; run \x1b[1mhelp oro\x1b[m for a list of commands")
