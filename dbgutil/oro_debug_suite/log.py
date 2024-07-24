def log(*args, **kwargs):
    """
    Oro-branded logging function.
    """
    print("\x1b[38;5;129moro\x1b[m", *args, **kwargs)


def warn(*args, **kwargs):
    """
    Oro-branded warning function.
    """
    # TODO(qix-): This adds a space between the last arg and the color reset message;
    # TODO(qix-): it really shouldn't. Just a minor nitpick.
    print("\x1b[38;5;129moro\x1b[38;5;220m", *args, "\x1b[m", **kwargs)


def error(*args, **kwargs):
    """
    Oro-branded error function.
    """
    # TODO(qix-): This adds a space between the last arg and the color reset message;
    # TODO(qix-): it really shouldn't. Just a minor nitpick.
    print("\x1b[38;5;129moro\x1b[38;5;160m", *args, "\x1b[m", **kwargs)
