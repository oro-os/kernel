import gdb  # type: ignore
from .service import QEMU


def gdb_prompt(_old_prompt):
    """
    Custom GDB prompt. Also runs a few commands that should run
    periodically.
    """

    QEMU.check_child()

    return "(oro-gdb) "


gdb.prompt_hook = gdb_prompt
