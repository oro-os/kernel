import gdb  # type: ignore
from ..log import debug, warn
from . import SYMBOLS, QEMU
from .autosym import (
    SYM_LOCK_ACQUIRE,
    SYM_LOCK_RELEASE,
)
from .backtrace import get_backtrace, warn_backtrace


class LockTracker(object):
    def __init__(self):
        self.__active_locks = dict()
        self.__seen_locks = set()
        self._lock_breakpoint = None
        self._release_breakpoint = None
        self.verbose = False
        self.__enabled = True

        SYMBOLS.on_loaded(self.attach)
        QEMU.on_started(self.clear)

    def clear(self, reattach=True):
        self.__active_locks.clear()
        self.__seen_locks.clear()
        debug("lock_tracker: cleared all lock acquisitions")
        if reattach:
            self.attach()

    @property
    def enabled(self):
        return self.__enabled

    @enabled.setter
    def enabled(self, value):
        self.__enabled = value
        self.attach()

    def get(self, addr):
        return self.__active_locks.get(addr, None)

    def _track_acquire(self, addr):
        bt = get_backtrace()

        self.__seen_locks.add(addr)

        if addr in self.__active_locks:
            warn(f"lock_tracker: double acquire: 0x{addr:016X}")
            warn_backtrace("lock_tracker", bt)
            warn(f"lock_tracker:    previous acquire:")
            warn_backtrace("lock_tracker", self.__active_locks[addr])
        else:
            if self.verbose:
                debug(f"lock_tracker: acquire: 0x{addr:016X}")
            self.__active_locks[addr] = bt

    def _track_release(self, addr):
        bt = get_backtrace()

        if addr not in self.__active_locks:
            if addr in self.__seen_locks:
                warn(f"lock_tracker: release without acquire: 0x{addr:016X}")
            else:
                warn(
                    f"lock_tracker: release on \x1b[1munseen\x1b[22m lock: 0x{addr:016X}"
                )
            warn_backtrace("lock_tracker", bt)
        else:
            if self.__active_locks[addr]["thread"] != bt["thread"]:
                warn(f"lock_tracker: release on different thread: 0x{addr:016X}")
                warn_backtrace("lock_tracker", bt)
                warn(f"lock_tracker:    previous acquire:")
                warn_backtrace("lock_tracker", self.__active_locks[addr])

            if self.verbose:
                debug(f"lock_tracker: release: 0x{addr:016X}")

            del self.__active_locks[addr]

    def attach(self):
        has_cleared = False
        if self._lock_breakpoint:
            self._lock_breakpoint.delete()
            self._lock_breakpoint = None
            has_cleared = True
        if self._release_breakpoint:
            self._release_breakpoint.delete()
            self._release_breakpoint = None
            has_cleared = True

        if has_cleared:
            debug("lock_tracker: detached")

        if self.enabled:
            acquire_sym = SYMBOLS.get_if_tracked(SYM_LOCK_ACQUIRE)
            release_sym = SYMBOLS.get_if_tracked(SYM_LOCK_RELEASE)
            if acquire_sym and release_sym:
                self._lock_breakpoint = LockTrackerAcquireBreakpoint(acquire_sym)
                self._release_breakpoint = LockTrackerReleaseBreakpoint(release_sym)
                debug("lock_tracker: attached")
            else:
                debug("lock_tracker: not attaching, missing symbols")


class LockTrackerAcquireBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(LockTrackerAcquireBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        arg = int(gdb.parse_and_eval("lock_self_addr_do_not_change_this_parameter"))
        LOCK_TRACKER._track_acquire(arg)
        return False  # don't stop


class LockTrackerReleaseBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(LockTrackerReleaseBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        arg = int(gdb.parse_and_eval("lock_self_addr_do_not_change_this_parameter"))
        LOCK_TRACKER._track_release(arg)
        return False  # don't stop


class LockEnableParam(gdb.Parameter):
    set_doc = "Enables/disables the Oro kernel lock tracker."
    show_doc = "Shows the current state of the Oro kernel lock tracker."

    def __init__(self):
        super(LockEnableParam, self).__init__(
            "oro-lock", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = LOCK_TRACKER.enabled

    def get_set_string(self):
        LOCK_TRACKER.enabled = self.value
        return ""


class LockVerboseParam(gdb.Parameter):
    set_doc = "Enables/disables verbose output for the Oro kernel lock tracker."
    show_doc = (
        "Shows the current state of verbose output for the Oro kernel lock tracker."
    )

    def __init__(self):
        super(LockVerboseParam, self).__init__(
            "oro-lock-verbose", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = LOCK_TRACKER.verbose

    def get_set_string(self):
        LOCK_TRACKER.verbose = self.value
        return ""


LOCK_TRACKER = LockTracker()

LockEnableParam()
LockVerboseParam()
