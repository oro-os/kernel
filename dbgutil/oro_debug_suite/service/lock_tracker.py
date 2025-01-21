import gdb  # type: ignore
from ..log import debug, warn
from . import SYMBOLS, QEMU
from .backtrace import get_backtrace, warn_backtrace
from .base import OroService, param, hook, service


@service("oro-lock", tag="lock_tracker")
class LockTracker(OroService):
    def __init__(self):
        super().__init__()
        self._active_locks = dict()
        self._seen_locks = set()

        self["enabled"] = False
        self["verbose"] = False

    def clear(self, reattach=True):
        super().clear(reattach)
        self._active_locks.clear()
        self._seen_locks.clear()
        self._debug("cleared all lock acquisitions")

    @classmethod
    def get(cls, addr):
        return self._instance._active_locks.get(addr, None)

    @classmethod
    def seen(cls, addr):
        return addr in self._instance._seen_locks

    @param
    def verbose(self, value):
        """Whether or not to show *every* lock acquire/release.
        !!! THIS IS VERY NOISY !!!"""
        pass

    @hook
    def lock_acquire(self, lock_self):
        bt = get_backtrace()

        self._seen_locks.add(lock_self)

        if lock_self in self._active_locks:
            self._warn(f"double acquire: 0x{lock_self:016X}")
            self._warn_backtrace(bt)
            self._warn(f"    previous acquire:")
            self._warn_backtrace(self._active_locks[lock_self])
        else:
            if self["verbose"]:
                self._debug(f"acquire: 0x{lock_self:016X}")
            self._active_locks[lock_self] = bt

    @hook
    def lock_release(self, lock_self):
        bt = get_backtrace()

        if lock_self not in self._active_locks:
            if lock_self in self._seen_locks:
                self._warn(f"release without acquire: 0x{lock_self:016X}")
            else:
                self._warn(f"release on \x1b[1munseen\x1b[22m lock: 0x{lock_self:016X}")
            self._warn_backtrace(bt)
        else:
            if self._active_locks[lock_self]["thread"] != bt["thread"]:
                self._warn(f"release on different thread: 0x{lock_self:016X}")
                self._warn_backtrace(bt)
                self._warn(f"    previous acquire:")
                self._warn_backtrace(self._active_locks[lock_self])

            if self.verbose:
                self._debug(f"release: 0x{lock_self:016X}")

            del self._active_locks[lock_self]
