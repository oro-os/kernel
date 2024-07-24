import gdb
from ..qemu import DEFAULT_ENDPOINT as QEMU_DEFAULT_ENDPOINT
from ..log import log, error
from ..service import QEMU


class QemuCmd(gdb.Command):
    """
    Interact with a QEMU monitor. Required for several high-level debugging operations.
    """

    def __init__(self):
        super(QemuCmd, self).__init__("oro qemu", gdb.COMMAND_USER, prefix=True)
        self._connection = None

    def invoke(self, arg, from_tty=False):
        gdb.execute("help qemu")
        pass


class QemuCmdConnect(gdb.Command):
    """
    Connect to a QEMU monitor.
    """

    def __init__(self):
        super(QemuCmdConnect, self).__init__("oro qemu connect", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        try:
            QEMU.connect(arg)
            log(f"qemu: connected to {QEMU.connection.endpoint}")
        except Exception as e:
            error(f"qemu: could not connect to {arg or QEMU_DEFAULT_ENDPOINT}: {e}")


class QemuCmdRaw(gdb.Command):
    """
    Send a raw command to the QEMU monitor and print the response.
    """

    def __init__(self):
        super(QemuCmdRaw, self).__init__("oro qemu raw", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        if not QEMU.is_connected:
            error("qemu: not connected; use 'qemu connect' to connect")
            return

        try:
            response = QEMU.connection.request(arg)
            log(response)
        except Exception as e:
            error(f"qemu: could not send command '{arg}': {e}")
            raise e


QemuCmd()
QemuCmdConnect()
QemuCmdRaw()
