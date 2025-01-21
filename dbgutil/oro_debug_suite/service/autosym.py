import gdb  # type: ignore
from ..log import debug

DEFAULT_TRACKED_SYMBOLS = frozenset(set(["oro_dbgutil::__oro_dbgutil_ATS1E1R"]))

_SYMBOL_FUNCTION_DOMAIN = (
    gdb.SYMBOL_FUNCTION_DOMAIN
    if hasattr(gdb, "SYMBOL_FUNCTION_DOMAIN")
    else gdb.SYMBOL_FUNCTIONS_DOMAIN
)


class SymbolTracker(object):
    def __init__(self):
        self.__to_track = set(DEFAULT_TRACKED_SYMBOLS)
        self.__symbols = dict()
        self.flush_all_on_new = False
        self.__on_loaded_events = set()

    def get(self, sym):
        sym = f"oro_dbgutil::__oro_dbgutil_{sym}"
        return self.__symbols.get(sym)

    def get_if_tracked(self, sym):
        sym = f"oro_dbgutil::__oro_dbgutil_{sym}"
        return sym if self.__symbols.get(sym) else None

    def on_loaded(self, callback):
        self.__on_loaded_events.add(callback)
        callback()

    def off_loaded(self, callback):
        self.__on_loaded_events.remove(callback)

    def _on_objfile_freed(self, objfile):
        for sym in self.__to_track:
            if sym in self.__symbols and self.__symbols[sym][1] == objfile:
                del self.__symbols[sym]

        for callback in self.__on_loaded_events:
            callback()

    def _on_objfile_loaded(self, objfile):
        self.read_elf_symbols(objfile.filename)

        if self.flush_all_on_new:
            self.__symbols.clear()

        for sym in self.__to_track:
            resolved = objfile.lookup_global_symbol(
                sym, _SYMBOL_FUNCTION_DOMAIN
            ) or objfile.lookup_static_symbol(sym, _SYMBOL_FUNCTION_DOMAIN)

            if resolved:
                address = int(resolved.value().address)
                self.__symbols[sym] = (address, objfile)
                debug(f"autosym: resolved {sym}=0x{address:016x}")

        for callback in self.__on_loaded_events:
            callback()

    def read_elf_symbols(self, elfpath):
        """Reads the **autosym-specific** hook symbols from the given kernel ELF.

        These symbols exist in the `.oro_dbgutil` section and are a list of null-terminated
        strings indicating the symbol names to track. Each of these is automatically prepended
        with `oro_dbgutil::` to form the full symbol name."""
        import p3elf.reader

        try:
            elf = p3elf.reader.ELFReader(elfpath)
            section = elf.get_section(".oro_dbgutil")
            new_syms = set(filter(lambda s: len(s) > 0, section.split(b"\0")))
        except p3elf.misc.NoSection as e:
            debug(f"autosym: no .oro_dbgutil section found in {elfpath}")
            return

        for new_sym in new_syms:
            sym = f"oro_dbgutil::{new_sym.decode('utf-8')}"
            if sym not in self.__to_track:
                debug(f"autosym: discovered exported hook symbol from ELF: {sym}")
                self.__to_track.add(sym)


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
