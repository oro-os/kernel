import gdb


def gdb_prompt(old_prompt):
    """
    Custom GDB prompt.
    """

    return "(oro) "


gdb.prompt_hook = gdb_prompt
