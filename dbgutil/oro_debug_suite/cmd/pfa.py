import gdb  # type: ignore
from ..log import log, error, warn


class PfaCmd(gdb.Command):
    """
    Manages the PFA tracker.

    The PFA tracker is turned off by default.

    The PFA has a few parameters that can be used to control its behavior:

        set oro-pfa on|off             Enables/disables the PFA tracker.
        set oro-pfa-verbose on|off     Enables/disables verbose output.
    """

    def __init__(self):
        super(PfaCmd, self).__init__("oro pfa", gdb.COMMAND_USER, prefix=True)

    def invoke(self, _arg, _from_tty=False):
        gdb.execute("help oro pfa")


PfaCmd()
