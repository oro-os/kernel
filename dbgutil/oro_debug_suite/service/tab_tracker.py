from .base import OroService, service, param, hook

# Must be in sync with the enum in tab.rs
TAB_TYPES = ["Free", "Ring", "Instance", "Thread", "RingInterface", "Module"]


@service("oro-tab", tag="tab_tracker")
class TabTracker(OroService):
    def __init__(self):
        self["enabled"] = False
        self["verbose"] = "off"
        self["locks"] = True

    @param(value={"on": True, "off": False, "full": "full"})
    def verbose(self, value):
        """Prints every tab operation that occurs.
        !!! THIS IS VERY NOISY !!!

        - off: No output.
        - on: Print tab CRUD operations and user add/remove.
        - full: All of 'on', plus lock acquire/release operations. !!! VERY NOISY !!!
        """
        pass

    @param
    def locks(self, value):
        """Track locks on tabs.
        !!! THIS IS NOISY WITH verbose=full AND SLOWS THINGS DOWN CONSIDERABLY !!!"""
        method = self.enable_breakpoint if value else self.disable_breakpoint
        method("tab_lock_read_acquire")
        method("tab_lock_read_release")
        method("tab_lock_write_acquire")
        method("tab_lock_write_release")

    @hook
    def tab_add(self, id, ty, slot_addr):
        if self["verbose"]:
            self._debug(
                f"tab_add: id={id:016x} ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x}"
            )

    @hook
    def tab_page_alloc(self, page, level):
        if self["verbose"]:
            self._debug(f"tab_page_alloc: page={page:016x} level={level}")

    @hook
    def tab_page_already_allocated(self, page, level):
        if self["verbose"]:
            self._debug(f"tab_page_already_allocated: page={page:016x} level={level}")

    @hook
    def tab_user_add(self, id, ty, slot_addr, prev_user_count):
        if self["verbose"]:
            self._debug(
                f"tab_user_add: id={id:016x} ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} prev_user_count={prev_user_count}"
            )

    @hook
    def tab_user_remove(self, id, ty, slot_addr, prev_user_count):
        if self["verbose"]:
            self._debug(
                f"tab_user_remove: id={id:016x} ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} prev_user_count={prev_user_count}"
            )

    @hook
    def tab_free(self, id, ty, slot_addr):
        if self["verbose"]:
            self._debug(
                f"tab_free: id={id:016x} ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x}"
            )

    @hook
    def tab_lock_read_acquire(self, ty, slot_addr, count, our_core):
        if self["verbose"] == "full":
            self._debug(
                f"tab_lock_read_acquire: ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} count={count} our_core={our_core}"
            )

    @hook
    def tab_lock_read_release(self, ty, slot_addr, count, our_core):
        if self["verbose"] == "full":
            self._debug(
                f"tab_lock_read_release: ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} count={count} our_core={our_core}"
            )

    @hook
    def tab_lock_write_acquire(self, ty, slot_addr, count, locked_core, our_core):
        if self["verbose"] == "full":
            self._debug(
                f"tab_lock_write_acquire: ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} count={count} locked_core={locked_core} our_core={our_core}"
            )

    @hook
    def tab_lock_write_release(self, ty, slot_addr, count, locked_core, our_core):
        if self["verbose"] == "full":
            self._debug(
                f"tab_lock_write_release: ty={TAB_TYPES[ty] or '???'} slot={slot_addr:016x} count={count} locked_core={locked_core} our_core={our_core}"
            )
