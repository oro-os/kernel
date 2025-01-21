import gdb  # type: ignore
from . import SYMBOLS, QEMU
from .backtrace import get_backtrace
from .base import OroService, service, param, hook


@service("oro-id", tag="core_id_tracker")
class CoreIdTracker(OroService):
    def __init__(self):
        # kernel ID => GDB core (thread) ID
        self.__oro_to_gdb = dict()
        # GDB core (thread) ID => kernel ID
        self.__gdb_to_oro = dict()

        self["enabled"] = False
        self["verbose"] = False

    def clear(self, reattach=True):
        self.__oro_to_gdb.clear()
        self.__gdb_to_oro.clear()
        self._debug("cleared all known core IDs")
        super().clear(reattach)

    def get_by_id(self, id):
        return self.__oro_to_gdb.get(id, None)

    def get_by_cpu(self, cpu):
        return self.__gdb_to_oro.get(cpu, None)

    @param
    def verbose(self, value):
        """Show every core ID funcion set and retrieval function call.
        !!! THIS IS VERY NOISY !!!"""
        pass

    @hook
    def core_id_fn_was_set(self, core_id):
        bt = get_backtrace()
        thread_id = bt["thread"]

        current_gdb = self.__oro_to_gdb.get(core_id, None)
        current_oro = self.__gdb_to_oro.get(thread_id, None)

        self.__oro_to_gdb[core_id] = (thread_id, bt)
        self.__gdb_to_oro[thread_id] = (core_id, bt)

        self._log(f"set: oro {core_id} ({hex(core_id)}) => gdb {thread_id}")
        self._log_backtrace(bt)

        if current_gdb is not None and current_gdb[0] != thread_id:
            self._warn(
                f"    ... above replaces existing known gdb core ID: oro {core_id} => WAS gdb {current_gdb[0]}, set at:"
            )
            self._warn_backtrace(current_gdb[1])
        if current_oro is not None and current_oro[0] != core_id:
            self._warn(
                f"    ... above replaces existing known oro core ID: WAS oro {current_oro[0]} => gdb {thread_id}, set at:"
            )
            self._warn_backtrace(current_oro[1])

    @hook
    def core_id_fn_was_called(self, core_id):
        bt = get_backtrace()
        thread_id = bt["thread"]

        current_gdb = self.__oro_to_gdb.get(core_id, None)
        current_oro = self.__gdb_to_oro.get(thread_id, None)

        if self["verbose"]:
            cgdb = None if current_gdb is None else current_gdb[0]
            coro = None if current_oro is None else current_oro[0]
            agree = (
                "AGREE" if cgdb == thread_id and coro == core_id else "!!! DISAGREE !!!"
            )
            self._debug(
                f"call: oro {core_id} (INTERNAL MAP => {cgdb}) ON gdb {thread_id} (INTERNAL MAP => {coro}) - {agree}"
            )

        if current_gdb is None:
            self._warn(
                f"call: unknown oro core ID: {core_id}, gdb {thread_id}, call at:"
            )
            self._warn_backtrace(bt)
        elif current_gdb[0] != thread_id:
            self._error(
                f"call: mismatched core IDs: oro {core_id} => gdb {current_gdb[0]}, but returned {thread_id}, call at:"
            )
            self._error_backtrace(bt)

        if current_oro is None:
            self._warn(
                f"call: unknown gdb core ID: gdb {thread_id}, oro {core_id}, call at:"
            )
            self._warn_backtrace(bt)
        elif current_oro[0] != core_id:
            self._error(
                f"call: mismatched core IDs: gdb {thread_id} => oro {current_oro[0]}, but returned {core_id}, call at:"
            )
            self._error_backtrace(bt)
