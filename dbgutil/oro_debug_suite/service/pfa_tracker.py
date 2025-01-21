from collections import defaultdict
import gdb  # type: ignore
from . import SYMBOLS, QEMU
from .backtrace import get_backtrace
from .base import OroService, service, param, hook


@service("oro-pfa", tag="pfa_tracker")
class PfaTracker(OroService):
    def __init__(self):
        self.__pfa_allocs = defaultdict(lambda: [])
        self._free_is_pfa_populating = False

        self["enabled"] = False
        self["verbose"] = False

    def clear(self, reattach=True):
        self.__pfa_allocs.clear()
        self._debug("cleared all mappings")
        super().clear(reattach)

    @param
    def verbose(self, value):
        """Show every allocation and free.
        !!! THIS IS VERY NOISY !!!"""
        pass

    @hook(symbol="pfa_alloc")
    def track_alloc_4kib(self, address):
        bt = get_backtrace()

        addr = address
        thread_id = bt["thread"]

        if self._free_is_pfa_populating:
            self._warn(f"allocation during PFA population event: 0x{addr:016X}")
            self._warn_backtrace(bt)

        events = self.__pfa_allocs[addr]

        if len(events) == 0:
            if self["verbose"]:
                self._debug(f"alloc: 0x{addr:016X} (first)")
        elif events[-1]["type"] == "alloc":
            self._warn(f"double alloc: 0x{addr:016X}")
            self._warn_backtrace(bt)
            self._warn(f"    previous alloc:")
            self._warn_backtrace(events[-1]["bt"])
        elif events[-1]["type"] == "free":
            if self["verbose"]:
                self._debug(f"alloc: 0x{addr:016X}")
        else:
            assert False, f"unknown allocation type: {events[-1]['type']}"

        events.append(
            {
                "type": "alloc",
                "bt": bt,
            }
        )

    @hook
    def pfa_free(self, address):
        if self._free_is_pfa_populating:
            self._warn("freeing during PFA population event; this should not happen!")
            self._warn_backtrace()
            return

        return self.track_free_4kib(address)

    def track_free_4kib(self, addr):
        bt = get_backtrace()

        thread_id = bt["thread"]

        if addr & 0xFFF:
            self._warn(f"freeing unaligned address: 0x{addr:016X}")
            self._warn_backtrace(bt)

        if self._free_is_pfa_populating:
            if addr in self.__pfa_allocs and len(self.__pfa_allocs[addr]) > 0:
                event = self.__pfa_allocs[addr][-1]
                if event["type"] == "alloc":
                    self._warn(
                        f"freeing an allocated page during PFA population event: 0x{addr:016X}"
                    )
                    self._warn_backtrace(bt)
                    self._warn(f"     previous allocation:")
                    self._warn_backtrace(event["bt"])
                elif event["type"] == "free":
                    self._warn(
                        f"double free during PFA population event: 0x{addr:016X}"
                    )
                    self._warn_backtrace(bt)
                    self._warn(f"     previous free:")
                    self._warn_backtrace(event["bt"])
                else:
                    assert False, f"unknown allocation type: {event['type']}"
            return

        events = self.__pfa_allocs[addr]
        if len(events) == 0:
            self._warn(f" freeing never-allocated page: 0x{addr:016X}")
            self._warn_backtrace(bt)
        elif events[-1]["type"] == "alloc":
            if self["verbose"]:
                self._debug(f"free: 0x{addr:016X}")
        else:
            self._warn(f"double free: 0x{addr:016X}")
            self._warn_backtrace(bt)
            self._warn(f"    previous free:")
            self._warn_backtrace(events[-1]["bt"])

        events.append(
            {
                "type": "free",
                "bt": bt,
            }
        )

    @hook(symbol="pfa_mass_free")
    def track_mass_free_4kib(self, start, end_exclusive):
        if self["verbose"]:
            self._debug(
                f"mass free: 0x{start:016X} - 0x{end_exclusive:016X} (exclusive)"
            )

        if start & 0xFFF:
            self._warn(f"mass free with unaligned start address: 0x{start:016X}")
            self._warn_backtrace(get_backtrace())

        if end_exclusive & 0xFFF:
            self._warn(
                f"mass free with unaligned end address: 0x{end_exclusive:016X} (exclusive)"
            )
            self._warn_backtrace(get_backtrace())

        # Just free the entire range; the `track_free_4kib()`
        # function handles the corner cases for e.g. a PFA
        # population event.
        for addr in range(start, end_exclusive, 0x1000):
            self.track_free_4kib(addr)

    @hook
    def pfa_will_mass_free(self, is_pfa_populating):
        if self["verbose"]:
            self._debug(
                f"kernel indicated it will mass free (pfa populating: {is_pfa_populating})"
            )
        if self._free_is_pfa_populating:
            self._warn(
                "kernel indicated it will mass free, but the free breakpoint is disabled; did it signal twice?"
            )
        self._free_is_pfa_populating = is_pfa_populating
        if not self.disable_breakpoint("pfa_free"):
            self._warn(
                "kernel indicated it will mass free, but the free breakpoint couldn't be found"
            )
            for _ in range(3):
                self._warn(
                    "!!! IT IS ABOUT TO GET VERY NOISY, OR VERY SLOW - OR BOTH !!!"
                )

    @hook
    def pfa_finished_mass_free(self):
        if self["verbose"]:
            self._debug(f"kernel indicated it finished a mass-free event")
        if not self._free_is_pfa_populating:
            self._warn(
                "kernel indicated it finished a mass free, but the free breakpoint is enabled; did it signal twice?"
            )
        self._free_is_pfa_populating = False
        if not self.enable_breakpoint("pfa_free"):
            self._error(
                "couldn't re-enable the free breakpoint after a mass free event"
            )
            for _ in range(3):
                self._error(
                    "!!! THE PFA TRACKER IS NO LONGER RELIABLE. PLEASE FIX THIS ISSUE. !!!"
                )
