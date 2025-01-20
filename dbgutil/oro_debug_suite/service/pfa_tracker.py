from collections import defaultdict
import gdb  # type: ignore
from ..log import debug, warn
from . import SYMBOLS, QEMU
from .backtrace import get_backtrace, warn_backtrace


class PfaTracker(object):
    def __init__(self):
        self.__pfa_allocs = defaultdict(lambda: [])
        self.__breakpoints = []
        self._free_breakpoint = None
        self.verbose = False
        self.__enabled = False
        self._free_is_pfa_populating = False

        SYMBOLS.on_loaded(self.attach)
        QEMU.on_started(self.clear)

    def clear(self, reattach=True):
        self.__pfa_allocs.clear()
        debug("pfa_tracker: cleared all mappings")

        if reattach:
            self.attach()

    @property
    def enabled(self):
        return self.__enabled

    @enabled.setter
    def enabled(self, value):
        self.__enabled = value
        self.attach()

    def track_alloc_4kib(self, addr):
        bt = get_backtrace()

        if self._free_is_pfa_populating:
            warn(f"pfa_tracker: allocation during PFA population event: 0x{addr:016X}")
            warn_backtrace("pfa_tracker", bt)

        events = self.__pfa_allocs[addr]
        if len(events) == 0:
            if self.verbose:
                debug(f"pfa_tracker: alloc: 0x{addr:016X} (first)")
        elif events[-1]["type"] == "alloc":
            warn(f"pfa_tracker: double alloc: 0x{addr:016X}")
            warn_backtrace("pfa_tracker", bt)
            warn(f"pfa_tracker:    previous alloc:")
            warn_backtrace("pfa_tracker", events[-1]["bt"])
        elif events[-1]["type"] == "free":
            if self.verbose:
                debug(f"pfa_tracker: alloc: 0x{addr:016X}")
        else:
            assert False, f"unknown allocation type: {events[-1]['type']}"

        events.append(
            {
                "type": "alloc",
                "bt": bt,
            }
        )

    def track_free_4kib(self, addr):
        bt = get_backtrace()

        if addr & 0xFFF:
            warn(f"pfa_tracker: freeing unaligned address: 0x{addr:016X}")
            warn_backtrace("pfa_tracker", bt)

        if self._free_is_pfa_populating:
            if addr in self.__pfa_allocs and len(self.__pfa_allocs[addr]) > 0:
                event = self.__pfa_allocs[addr][-1]
                if event["type"] == "alloc":
                    warn(
                        f"pfa_tracker: freeing an allocated page during PFA population event: 0x{addr:016X}"
                    )
                    warn_backtrace("pfa_tracker", bt)
                    warn(f"pfa_tracker:     previous allocation:")
                    warn_backtrace("pfa_tracker", event["bt"])
                elif event["type"] == "free":
                    warn(
                        f"pfa_tracker: double free during PFA population event: 0x{addr:016X}"
                    )
                    warn_backtrace("pfa_tracker", bt)
                    warn(f"pfa_tracker:     previous free:")
                    warn_backtrace("pfa_tracker", event["bt"])
                else:
                    assert False, f"unknown allocation type: {event['type']}"
            return

        events = self.__pfa_allocs[addr]
        if len(events) == 0:
            warn(f"pfa_tracker: freeing never-allocated page: 0x{addr:016X}")
            warn_backtrace("pfa_tracker", bt)
        elif events[-1]["type"] == "alloc":
            if self.verbose:
                debug(f"pfa_tracker: free: 0x{addr:016X}")
        else:
            warn(f"pfa_tracker: double free: 0x{addr:016X}")
            warn_backtrace("pfa_tracker", bt)
            warn(f"pfa_tracker:    previous free:")
            warn_backtrace("pfa_tracker", events[-1]["bt"])

        events.append(
            {
                "type": "free",
                "bt": bt,
            }
        )

    def track_mass_free_4kib(self, start, end_excl):
        if self.verbose:
            debug(
                f"pfa_tracker: mass free: 0x{start:016X} - 0x{end_excl:016X} (exclusive)"
            )

        if start & 0xFFF:
            warn(f"pfa_tracker: mass free with unaligned start address: 0x{start:016X}")
            warn_backtrace("pfa_tracker", get_backtrace())

        if end_excl & 0xFFF:
            warn(
                f"pfa_tracker: mass free with unaligned end address: 0x{end_excl:016X} (exclusive)"
            )
            warn_backtrace("pfa_tracker", get_backtrace())

        # Just free the entire range; the `track_free_4kib()`
        # function handles the corner cases for e.g. a PFA
        # population event.
        for addr in range(start, end_excl, 0x1000):
            self.track_free_4kib(addr)

    def attach(self):
        has_cleared = False
        if self._free_breakpoint:
            self._free_breakpoint.delete()
            self._free_breakpoint = None
            has_cleared = True
        for bp in self.__breakpoints:
            bp.delete()
            has_cleared = True
        self.__breakpoints.clear()

        if has_cleared:
            debug("pfa_tracker: detached")

        if self.enabled:
            free_sym = SYMBOLS.get_if_tracked("pfa_free")
            alloc_sym = SYMBOLS.get_if_tracked("pfa_alloc")
            will_mass_free_sym = SYMBOLS.get_if_tracked("pfa_will_mass_free")
            finished_mass_free_sym = SYMBOLS.get_if_tracked("pfa_finished_mass_free")
            mass_free_sym = SYMBOLS.get_if_tracked("pfa_mass_free")
            if all(
                [
                    free_sym,
                    alloc_sym,
                    will_mass_free_sym,
                    finished_mass_free_sym,
                    mass_free_sym,
                ]
            ):
                self.__breakpoints.append(PfaTrackerAllocBreakpoint(alloc_sym))
                self.__breakpoints.append(
                    PfaTrackerWillMassFreeBreakpoint(will_mass_free_sym)
                )
                self.__breakpoints.append(
                    PfaTrackerFinishedMassFreeBreakpoint(finished_mass_free_sym)
                )
                self.__breakpoints.append(PfaTrackerMassFreeBreakpoint(mass_free_sym))
                self._free_breakpoint = PfaTrackerFreeBreakpoint(free_sym)
                debug("pfa_tracker: attached")
            else:
                debug("pfa_tracker: not attaching, missing symbols")


class PfaTrackerAllocBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(PfaTrackerAllocBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        arg = int(gdb.parse_and_eval("address"))
        PFA_TRACKER.track_alloc_4kib(arg)
        return False  # don't stop


class PfaTrackerFreeBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(PfaTrackerFreeBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        arg = int(gdb.parse_and_eval("address"))
        PFA_TRACKER.track_free_4kib(arg)
        return False  # don't stop


class PfaTrackerWillMassFreeBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(PfaTrackerWillMassFreeBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        is_pfa_populating = bool(int(gdb.parse_and_eval("is_pfa_populating")))
        if PFA_TRACKER.verbose:
            debug(
                f"pfa_tracker: kernel indicated it will mass free (pfa populating: {is_pfa_populating})"
            )
        if not PFA_TRACKER._free_breakpoint.enabled:
            warn(
                "pfa_tracker: kernel indicated it will mass free, but the free breakpoint is disabled; did it signal twice?"
            )
        PFA_TRACKER._free_breakpoint.enabled = False
        PFA_TRACKER._free_is_pfa_populating = is_pfa_populating
        return False  # don't stop


class PfaTrackerFinishedMassFreeBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(PfaTrackerFinishedMassFreeBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        if PFA_TRACKER.verbose:
            debug(f"pfa_tracker: kernel indicated it finished a mass-free event")
        if PFA_TRACKER._free_breakpoint.enabled:
            warn(
                "pfa_tracker: kernel indicated it finished a mass free, but the free breakpoint is enabled; did it signal twice?"
            )
        PFA_TRACKER._free_breakpoint.enabled = True
        PFA_TRACKER._free_is_pfa_populating = False
        return False  # don't stop


class PfaTrackerMassFreeBreakpoint(gdb.Breakpoint):
    def __init__(self, at):
        super(PfaTrackerMassFreeBreakpoint, self).__init__(
            at, internal=True, qualified=True
        )

    def stop(self):
        start = int(gdb.parse_and_eval("start"))
        end_excl = int(gdb.parse_and_eval("end_exclusive"))
        PFA_TRACKER.track_mass_free_4kib(start, end_excl)
        return False  # don't stop


class PfaEnableParam(gdb.Parameter):
    set_doc = "Enables/disables the Oro kernel PFA tracker."
    show_doc = "Shows the current state of the Oro kernel PFA tracker."

    def __init__(self):
        super(PfaEnableParam, self).__init__(
            "oro-pfa", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = PFA_TRACKER.enabled

    def get_set_string(self):
        PFA_TRACKER.enabled = self.value
        return ""


class PfaVerboseParam(gdb.Parameter):
    set_doc = "Enables/disables verbose output for the Oro kernel PFA tracker."
    show_doc = (
        "Shows the current state of verbose output for the Oro kernel PFA tracker."
    )

    def __init__(self):
        super(PfaVerboseParam, self).__init__(
            "oro-pfa-verbose", gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN
        )
        self.value = PFA_TRACKER.verbose

    def get_set_string(self):
        PFA_TRACKER.verbose = self.value
        return ""


PFA_TRACKER = PfaTracker()

PfaEnableParam()
PfaVerboseParam()
