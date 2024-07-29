import gdb  # type: ignore
from ..service import QEMU
from ..log import error, log


class QemuCmd(gdb.Command):
    """
    Interact with a running QEMU session.
    """

    def __init__(self):
        super(QemuCmd, self).__init__("oro qemu", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro qemu")


class QemuCmdMon(gdb.Command):
    """
    Sends a "human" monitor command to QEMU.

    This is the same as running a command in the QEMU monitor.

    Usage: qemu monitor <command>
    """

    def __init__(self):
        super(QemuCmdMon, self).__init__("oro qemu monitor", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty=False):
        arg = arg.strip()
        if not arg:
            error("qemu monitor: no command specified")
            return

        # Will raise an exception if QEMU is not running
        qemu = QEMU.session

        response = qemu.monitor_command(arg).rstrip()

        for line in response.split("\r\n"):
            log(f"qemu: {line}")


QemuCmd()
QemuCmdMon()
