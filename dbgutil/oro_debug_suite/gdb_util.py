import gdb  # type: ignore


class parameter(object):
    """
    Allows setting a GDB parameter temporarily using a context manager (`with` statement).
    """

    def __init__(self, param, value):
        self.__param = param
        self.__value = value
        self.__original = None

    @property
    def original(self):
        if self.__original is None:
            return gdb.parameter(self.__param)
        else:
            return self.__original

    def __enter__(self):
        self.__original = gdb.parameter(self.__param)
        gdb.set_parameter(self.__param, self.__value)

    def __exit__(self, *args, **kwargs):
        gdb.set_parameter(self.__param, self.__original)
        self.__original = None
