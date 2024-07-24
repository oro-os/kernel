import gdb  # type: ignore


def gdb_prompt(_old_prompt):
    """
    Custom GDB prompt.
    """

    return "(oro) "


gdb.prompt_hook = gdb_prompt
