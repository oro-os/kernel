import gdb  # type: ignore
from ..log import warn, log
from ..qemu import QemuProcess


class QemuService(object):
    """
    Manages a singleton instance of QEMU.
    """

    def __init__(self):
        self._child = None

    @property
    def running(self):
        """
        Returns the active QEMU session

        Raises an exception if one is not started.
        """

        if not self._child:
            raise Exception("QEMU is not running; use 'oro boot' to spawn the kernel")

        return self._child

    @property
    def is_running(self):
        """
        Is QEMU running?
        """

        return self._child is not None

    def spawn_and_connect(self, args, **kwargs):
        """
        Spawns QEMU with the given arguments and connects GDB to it.
        """

        if self.is_running:
            warn("QEMU is already running; stopping it before starting a new instance")
            self.shutdown()
            del self._child
            self._child = None

        log("spawning QEMU...")
        self._child = QemuProcess(args, **kwargs)
        log("connecting to QEMU gdbserver...")
        self._child.connect_gdb()
        log("QEMU started")
        return self._child

    def check_child(self):
        if not self.is_running:
            return

        r = self._child.poll()
        if r is not None:
            warn(f"QEMU exited with code \x1b[1m{r}\x1b[22m; cleaning up...")
            self.shutdown()

    def shutdown(self):
        if self.is_running:
            log("shutting down QEMU...")
            self._child.shutdown()
            log("QEMU stopped")
            del self._child
            self._child = None


QEMU = QemuService()
gdb.events.gdb_exiting.connect(lambda *args: QEMU.shutdown())
gdb.events.stop.connect(lambda *args: QEMU.check_child())
