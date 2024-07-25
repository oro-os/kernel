class Backend(object):
    """
    Base class for debug suite backends (QEMU, etc).

    Provides higher level operations for interacting with the backend,
    keeping the debug commands agnostic of the underlying implementation.

    These are operations not possible to perform with GDB's built-in
    commands, such as reading physical memory, device trees, etc.
    """

    def __unimplemented(self, name):
        raise NotImplementedError(
            f"backend '{self.__class__.__name__}' does not support the '{name}' operation"
        )

    def read_physical(self, addr, length):
        """
        Read physical memory from the target.

        `addr` is a *physical* address to read from.
        Returns a bytes object, or `None` if the address is invalid.
        Raises an exception if the operation fails.
        """

        self.__unimplemented("read_physical")
