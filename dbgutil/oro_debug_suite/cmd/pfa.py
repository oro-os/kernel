import gdb  # type: ignore
from ..log import log, error, warn


class PfaCmd(gdb.Command):
    """
    Manages the PFA tracker.

    The PFA tracker is turned on by default, but with no verbose output.

    The PFA has a few parameters that can be used to control its behavior:

        set oro-pfa on|off             Enables/disables the PFA tracker.
        set oro-pfa-verbose on|off     Enables/disables verbose output.
    """

    def __init__(self):
        super(PfaCmd, self).__init__("oro pfa", gdb.COMMAND_USER, prefix=True)

    def invoke(self, _arg, _from_tty=False):
        gdb.execute("help oro pfa")


class PfaEnableCmd(gdb.Command):
    """
    Enables the PFA tracker.

    Alternatively, you can use `set oro-pfa on` to enable the PFA tracker.
    This command simply invokes that.
    """

    def __init__(self):
        super(PfaEnableCmd, self).__init__("oro pfa enable", gdb.COMMAND_USER)

    def invoke(self, _arg, _from_tty=False):
        gdb.execute("set oro-pfa on")


class PfaDisableCmd(gdb.Command):
    """
    Disable the PFA tracker.

    Alternatively, you can use `set oro-pfa off` to disable the PFA tracker.
    This command simply invokes that.
    """

    def __init__(self):
        super(PfaDisableCmd, self).__init__("oro pfa disable", gdb.COMMAND_USER)

    def invoke(self, _arg, _from_tty=False):
        gdb.execute("set oro-pfa off")


PfaCmd()
PfaEnableCmd()
PfaDisableCmd()
