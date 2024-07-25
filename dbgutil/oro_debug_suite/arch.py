import gdb  # type: ignore


def get_arch():
    """
    Returns the name of the architecture of the currently selected inferior.

    Raises an exception if no inferior is selected.
    """

    inferior = gdb.selected_inferior()
    if not inferior:
        raise Exception("No inferior selected; cannot get architecture")

    return inferior.architecture().name()
