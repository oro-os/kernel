import gdb  # type: ignore
from ..service import QEMU
from ..log import error, log


class TtCmd(gdb.Command):
    """
    Memory translation utilities.
    """

    def __init__(self):
        super(TtCmd, self).__init__("oro tt", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro tt")


class TtCmdVirt(gdb.Command):
    """
    Translate a virtual address to a physical address.

    Usage: oro tt virt <address>
    """

    def __init__(self):
        super(TtCmdVirt, self).__init__("oro tt virt", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro tt virt")
            return

        try:
            virt = int(args[0], 0)
        except ValueError:
            gdb.execute("help oro tt virt")
            return

        if virt < 0:
            gdb.execute("help oro tt virt")
            return

        # Guaranteed to be connected; would throw if we weren't.
        qemu = QEMU.connection

        inferior = gdb.selected_inferior()
        if not inferior:
            error("tt: no inferior selected")
            return

        arch = inferior.architecture().name()
        log(f"arch={arch}")

        if arch == "aarch64":
            pass
        else:
            error("tt: unsupported architecture")


TtCmd()
TtCmdVirt()
