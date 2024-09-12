import gdb  # type: ignore
from ..log import log, error, warn
from ..service.lock_tracker import LOCK_TRACKER
from ..service.backtrace import log_backtrace


class LockCmd(gdb.Command):
    """
    Manages the lock tracker.

    The lock tracker is turned on by default, but with no verbose output.

    The lock has a few parameters that can be used to control its behavior:

        set oro-lock on|off             Enables/disables the lock tracker.
        set oro-lock-verbose on|off     Enables/disables verbose output.
    """

    def __init__(self):
        super(LockCmd, self).__init__("oro lock", gdb.COMMAND_USER, prefix=True)

    def invoke(self, _arg, _from_tty=False):
        gdb.execute("help oro lock")


class LockCmdStatus(gdb.Command):
    """
    Shows the current status of a lock.
    """

    def __init__(self):
        super(LockCmdStatus, self).__init__(
            "oro lock status", gdb.COMMAND_USER, prefix=True
        )

    def invoke(self, arg, _from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro lock status")
            return

        lock_expr = args[0]
        lock_addr = int(gdb.parse_and_eval(lock_expr))

        maybe_lock = LOCK_TRACKER.get(lock_addr)

        if maybe_lock is None:
            if LOCK_TRACKER.seen(lock_addr):
                log(
                    f"lock_tracker: lock at 0x{lock_addr:016X} is \x1b[1mreleased\x1b[22m"
                )
            else:
                warn(
                    f"lock_tracker: lock at 0x{lock_addr:016X} is \x1b[1munseen\x1b[22m (either never acquired or expression is not a lock)"
                )
        else:
            log(f"lock_tracker: lock at 0x{lock_addr:016X} is \x1b[1mlocked\x1b[22m")
            log_backtrace("lock_tracker", maybe_lock)


LockCmd()
LockCmdStatus()
