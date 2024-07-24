def log(*args, **kwargs):
    """
    Oro-branded logging function.
    """
    print("\x1b[38;5;129moro\x1b[m", *args, **kwargs)
