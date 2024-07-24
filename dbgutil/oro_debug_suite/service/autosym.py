import gdb  # type: ignore
from ..log import log

TRACKED_SYMBOLS = frozenset(
    set(
        [
            # AArch64: AT S1E1R instruction stub
            ("f", "oro_arch_aarch64::dbgutil::__oro_dbgutil_ATS1E1R")
        ]
    )
)

_DOMAINS = {"f": gdb.SYMBOL_FUNCTION_DOMAIN, "v": gdb.SYMBOL_VAR_DOMAIN}


class SymbolTracker(object):
    def __init__(self):
        self.__symbols = {}

    @property
    def symbols(self):
        return frozenset(self.__symbols)

    def _on_objfile_freed(self, objfile):
        for _, sym in TRACKED_SYMBOLS:
            if sym in self.__symbols and self.__symbols[sym][1] == objfile:
                del self.__symbols[sym]

    def _on_objfile_loaded(self, objfile):
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
                log(f"autosym: resolved {sym}=0x{address:#016x}")


SYMBOLS = SymbolTracker()

for objfile in gdb.objfiles():
    SYMBOLS._on_objfile_loaded(objfile)

gdb.events.new_objfile.connect(
    lambda event: SYMBOLS._on_objfile_loaded(event.new_objfile)
)
gdb.events.free_objfile.connect(lambda event: SYMBOLS._on_objfile_freed(event.objfile))
