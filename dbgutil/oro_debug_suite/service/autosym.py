import gdb  # type: ignore
from ..log import debug

## AArch64: AT S1E1R instruction stub
SYM_AARCH64_ATS1E1R = "oro_debug::__oro_dbgutil_ATS1E1R"
## All: Transfer to kernel function hook
SYM_KERNEL_TRANSFER = "oro_debug::__oro_dbgutil_kernel_will_transfer"
## All: Page frame allocation hook (4KiB page)
SYM_PAGE_ALLOC = "oro_debug::__oro_dbgutil_pfa_alloc"
## All: Page frame free hook (4KiB page)
SYM_PAGE_FREE = "oro_debug::__oro_dbgutil_pfa_free"
## All: The kernel is about to free a lot of pages;
##      the PFA tracker expects that the kernel will then call
##      SYM_PFA_MASS_FREE zero or more times, followed by
##      SYM_PFA_FINISHED_MASS_FREE.
SYM_PFA_WILL_MASS_FREE = "oro_debug::__oro_dbgutil_pfa_will_mass_free"
## All: The kernel has finished freeing a lot of pages.
SYM_PFA_FINISHED_MASS_FREE = "oro_debug::__oro_dbgutil_pfa_finished_mass_free"
## All: Indicates that a region of memory has been freed.
##      This is used by the kernel when performing the initial
##      population of the PFA. It'll most likely go away in the future
##      when the PFA supports regions.
SYM_PFA_MASS_FREE = "oro_debug::__oro_dbgutil_pfa_mass_free"

TRACKED_SYMBOLS = frozenset(
    set(
        [
            ("f", SYM_AARCH64_ATS1E1R),
            ("f", SYM_KERNEL_TRANSFER),
            ("f", SYM_PAGE_ALLOC),
            ("f", SYM_PAGE_FREE),
            ("f", SYM_PFA_WILL_MASS_FREE),
            ("f", SYM_PFA_FINISHED_MASS_FREE),
            ("f", SYM_PFA_MASS_FREE),
        ]
    )
)

SYMBOL_FUNCTION_DOMAIN = (
    gdb.SYMBOL_FUNCTION_DOMAIN
    if hasattr(gdb, "SYMBOL_FUNCTION_DOMAIN")
    else gdb.SYMBOL_FUNCTIONS_DOMAIN
)

_DOMAINS = {"f": SYMBOL_FUNCTION_DOMAIN, "v": gdb.SYMBOL_VAR_DOMAIN}


class SymbolTracker(object):
    def __init__(self):
        self.__symbols = dict()
        self.flush_all_on_new = False
        self.__on_loaded_events = set()

    def get(self, sym):
        return self.__symbols.get(sym)

    def get_if_tracked(self, sym):
        return sym if self.get(sym) else None

    def on_loaded(self, callback):
        self.__on_loaded_events.add(callback)
        callback()

    def off_loaded(self, callback):
        self.__on_loaded_events.remove(callback)

    def _on_objfile_freed(self, objfile):
        for _, sym in TRACKED_SYMBOLS:
            if sym in self.__symbols and self.__symbols[sym][1] == objfile:
                del self.__symbols[sym]

        for callback in self.__on_loaded_events:
            callback()

    def _on_objfile_loaded(self, objfile):
        if self.flush_all_on_new:
            self.__symbols.clear()

        for domain, sym in TRACKED_SYMBOLS:
            assert bool(
                _DOMAINS[domain]
            ), f"invalid domain: {domain} (this is a bug in Oro dbgutil)"
            domain = _DOMAINS[domain]

            resolved = objfile.lookup_global_symbol(
                sym, domain
            ) or objfile.lookup_static_symbol(sym, domain)

            if resolved:
                address = int(resolved.value().address)
                self.__symbols[sym] = (address, objfile)
                debug(f"autosym: resolved {sym}=0x{address:016x}")

        for callback in self.__on_loaded_events:
            callback()


SYMBOLS = SymbolTracker()

for objfile in gdb.objfiles():
    SYMBOLS._on_objfile_loaded(objfile)

gdb.events.new_objfile.connect(
    lambda event: SYMBOLS._on_objfile_loaded(event.new_objfile)
)

if hasattr(gdb.events, "free_objfile"):
    gdb.events.free_objfile.connect(
        lambda event: SYMBOLS._on_objfile_freed(event.objfile)
    )
else:
    SYMBOLS.flush_all_on_new = True
