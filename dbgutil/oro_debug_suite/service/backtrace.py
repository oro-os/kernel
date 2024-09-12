import gdb  # type: ignore
from ..log import warn, log, error
from .thread_colors import THREAD_COLORS


def get_backtrace():
    thread = gdb.selected_thread().num
    frame = gdb.newest_frame()
    frames = []
    while frame:
        sal = frame.find_sal()
        frames.append(
            {
                "function": frame.function(),
                "pc": sal.pc,
                "line": sal.line,
                "filename": sal.symtab.filename if sal.symtab else None,
            }
        )
        frame = frame.older()
    return {
        "thread": thread,
        "frames": frames,
    }


def __print_backtrace(fn, tag, bt):
    fn(
        f"{tag}:         on GDB thread \x1b[38;5;{THREAD_COLORS[bt['thread']-1]}m{bt['thread']}"
    )
    if len(bt["frames"]) > 0:
        for frame in bt["frames"]:
            fn(
                f"{tag}:         at {frame.get('filename', '<unknown filename>')}:{frame['line']}"
            )
            fn(
                f"{tag}:            \x1b[2m{frame.get('function', '<unknown fn>')} (0x{frame.get('pc', 0):016X})\x1b[22m"
            )


def log_backtrace(tag, bt):
    __print_backtrace(log, tag, bt)


def warn_backtrace(tag, bt):
    __print_backtrace(warn, tag, bt)


def error_backtrace(tag, bt):
    __print_backtrace(error, tag, bt)
