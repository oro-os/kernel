import gdb


def gdb_prompt(_old_prompt):
    """
    Custom GDB prompt.
    """

    return "\x1b[38;5;082m(oro)\x1b[m "


gdb.prompt_hook = gdb_prompt
