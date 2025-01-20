import gdb  # type: ignore
from ..log import debug, warn, error, log
from . import SYMBOLS, QEMU
from .backtrace import get_backtrace, warn_backtrace, log_backtrace, error_backtrace


class LockTracker(object):
    def __init__(self):
        # kernel ID => GDB core (thread) ID
        self.__oro_to_gdb = dict()
        # GDB core (thread) ID => kernel ID
        self.__gdb_to_oro = dict()
        self.verbose = False
        self.__enabled = False
        self._set_breakpoint = None
        self._call_breakpoint = None

        SYMBOLS.on_loaded(self.attach)
        QEMU.on_started(self.clear)

    def clear(self, reattach=True):
        self.__oro_to_gdb.clear()
        self.__gdb_to_oro.clear()
        debug("core_id_tracker: cleared all known core IDs")
        if reattach:
            self.attach()

    @property
    def enabled(self):
        return self.__enabled

    @enabled.setter
    def enabled(self, value):
        self.__enabled = value
        self.attach()

    def get_by_id(self, id):
        return self.__oro_to_gdb.get(id, None)

    def get_by_cpu(self, cpu):
        return self.__gdb_to_oro.get(cpu, None)

    def _track_set(self, core_id, thread_id):
        bt = get_backtrace()

        current_gdb = self.__oro_to_gdb.get(core_id, None)
        current_oro = self.__gdb_to_oro.get(thread_id, None)

        self.__oro_to_gdb[core_id] = (thread_id, bt)
        self.__gdb_to_oro[thread_id] = (core_id, bt)

        log(f"core_id_tracker: set: oro {core_id} ({hex(core_id)}) => gdb {thread_id}")
        log_backtrace("core_id_tracker", bt)

        if current_gdb is not None and current_gdb[0] != thread_id:
            warn(
                f"core_id_tracker:    ... above replaces existing known gdb core ID: oro {core_id} => WAS gdb {current_gdb[0]}, set at:"
            )
            warn_backtrace("core_id_tracker", current_gdb[1])
        if current_oro is not None and current_oro[0] != core_id:
            warn(
                f"core_id_tracker:    ... above replaces existing known oro core ID: WAS oro {current_oro[0]} => gdb {thread_id}, set at:"
            )
            warn_backtrace("core_id_tracker", current_oro[1])

    def _track_call(self, core_id, thread_id):
        bt = get_backtrace()

        current_gdb = self.__oro_to_gdb.get(core_id, None)
        current_oro = self.__gdb_to_oro.get(thread_id, None)

        if self.verbose:
            cgdb = None if current_gdb is None else current_gdb[0]
            coro = None if current_oro is None else current_oro[0]
            agree = (
                "AGREE" if cgdb == thread_id and coro == core_id else "!!! DISAGREE !!!"
            )
            debug(
                f"core_id_tracker: call: oro {core_id} (INTERNAL MAP => {cgdb}) ON gdb {thread_id} (INTERNAL MAP => {coro}) - {agree}"
            )

        if current_gdb is None:
            warn(
                f"core_id_tracker: call: unknown oro core ID: {core_id}, gdb {thread_id}, call at:"
            )
            warn_backtrace("core_id_tracker", bt)
        elif current_gdb[0] != thread_id:
            error(
                f"core_id_tracker: call: mismatched core IDs: oro {core_id} => gdb {current_gdb[0]}, but returned {thread_id}, call at:"
            )
            error_backtrace("core_id_tracker", bt)

        if current_oro is None:
            warn(
                f"core_id_tracker: call: unknown gdb core ID: gdb {thread_id}, oro {core_id}, call at:"
            )
            warn_backtrace("core_id_tracker", bt)
        elif current_oro[0] != core_id:
            error(
                f"core_id_tracker: call: mismatched core IDs: gdb {thread_id} => oro {current_oro[0]}, but returned {core_id}, call at:"
            )
            error_backtrace("core_id_tracker", bt)

    def attach(self):
        has_cleared = False
        if self._set_breakpoint:
            self._set_breakpoint.delete()
            self._set_breakpoint = None
            has_cleared = True
        if self._call_breakpoint:
            self._call_breakpoint.delete()
            self._call_breakpoint = None
            has_cleared = True

        if has_cleared:
            debug("core_id_tracker: detached")

        if self.enabled:
            set_sym = SYMBOLS.get_if_tracked("core_id_fn_was_set")
            call_sym = SYMBOLS.get_if_tracked("core_id_fn_was_called")
            if set_sym and call_sym:
                self._set_breakpoint = CoreIdTrackerSetBreakpoint(set_sym)
                self._call_breakpoint = CoreIdTrackerCallBreakpoint(call_sym)
                debug("core_id_tracker: attached")
            else:
                debug("core_id_tracker: not attached, missing symbols")


class CoreIdTrackerSetBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(CoreIdTrackerSetBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        core_id = int(gdb.parse_and_eval("core_id"))
        thread_id = gdb.selected_thread().num
        CORE_ID_TRACKER._track_set(core_id, thread_id)
        return False  # don't stop


class CoreIdTrackerCallBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(CoreIdTrackerCallBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        core_id = int(gdb.parse_and_eval("core_id"))
        thread_id = gdb.selected_thread().num
        CORE_ID_TRACKER._track_call(core_id, thread_id)
        return False  # don't stop


class CoreIdEnableParam(gdb.Parameter):
    set_doc = "Enables/disables the Oro kernel core ID tracker."
    show_doc = "Shows the current state of the Oro kernel core ID tracker."

    def __init__(self):
        super(CoreIdEnableParam, self).__init__(
            "oro-core-id", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = CORE_ID_TRACKER.enabled

    def get_set_string(self):
        CORE_ID_TRACKER.enabled = self.value
        return ""


class CoreIdVerboseParam(gdb.Parameter):
    set_doc = "Enables/disables verbose output for the Oro kernel core ID tracker."
    show_doc = (
        "Shows the current state of verbose output for the Oro kernel core ID tracker."
    )

    def __init__(self):
        super(CoreIdVerboseParam, self).__init__(
            "oro-core-id-verbose", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = CORE_ID_TRACKER.verbose

    def get_set_string(self):
        CORE_ID_TRACKER.verbose = self.value
        return ""


CORE_ID_TRACKER = LockTracker()

CoreIdEnableParam()
CoreIdVerboseParam()
