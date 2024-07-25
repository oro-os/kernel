import gdb  # type: ignore
from ..service import QEMU, SYMBOLS
from ..service.autosym import SYM_AARCH64_ATS1E1R
from ..log import error, log, warn

_FAULT_STATUSES = {
    "0b000000": "address size fault, level 0 of translation or translation table base register",
    "0b000001": "address size fault, level 1",
    "0b000010": "address size fault, level 2",
    "0b000011": "address size fault, level 3",
    "0b000100": "translation fault, level 0",
    "0b000101": "translation fault, level 1",
    "0b000110": "translation fault, level 2",
    "0b000111": "translation fault, level 3",
    "0b001001": "access flag fault, level 1",
    "0b001010": "access flag fault, level 2",
    "0b001011": "access flag fault, level 3",
    "0b001000": "access flag fault, level 0",
    "0b001100": "permission fault, level 0",
    "0b001101": "permission fault, level 1",
    "0b001110": "permission fault, level 2",
    "0b001111": "permission fault, level 3",
    "0b010011": "synchronous External abort on translation table walk or hardware update of translation table, level -1",
    "0b010100": "synchronous External abort on translation table walk or hardware update of translation table, level 0",
    "0b010101": "synchronous External abort on translation table walk or hardware update of translation table, level 1",
    "0b010110": "synchronous External abort on translation table walk or hardware update of translation table, level 2",
    "0b010111": "synchronous External abort on translation table walk or hardware update of translation table, level 3",
    "0b011011": "synchronous parity or ECC error on memory access on translation table walk or hardware update of translation table, level -1",
    "0b011100": "synchronous parity or ECC error on memory access on translation table walk or hardware update of translation table, level 0",
    "0b011101": "synchronous parity or ECC error on memory access on translation table walk or hardware update of translation table, level 1",
    "0b011110": "synchronous parity or ECC error on memory access on translation table walk or hardware update of translation table, level 2",
    "0b011111": "synchronous parity or ECC error on memory access on translation table walk or hardware update of translation table, level 3",
    "0b101001": "address size fault, level -1",
    "0b101011": "translation fault, level -1",
    "0b110000": "TLB conflict abort",
    "0b110001": "unsupported atomic hardware update fault",
    "0b111101": "section Domain fault, from an AArch32 stage 1 EL1&0 translation regime using Short-descriptor translation table format",
    "0b111110": "page Domain fault, from an AArch32 stage 1 EL1&0 translation regime using Short-descriptor translation table format",
}

_SHAREABILITY = [
    "Non-shareable",
    "Reserved (invalid shareability)",
    "Outer Shareable / Device",
    "Inner Shareable",
]


class TtCmd(gdb.Command):
    """
    Memory translation utilities.
    """

    def __init__(self):
        super(TtCmd, self).__init__("oro tt", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro tt")


class TtCmdVirt(gdb.Command):
    """
    Translate a virtual address to a physical address.

    Usage: oro tt virt <address>
    """

    def __init__(self):
        super(TtCmdVirt, self).__init__("oro tt virt", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro tt virt")
            return

        try:
            virt = int(args[0], 0)
        except ValueError:
            gdb.execute("help oro tt virt")
            return

        if virt < 0:
            gdb.execute("help oro tt virt")
            return

        # Guaranteed to be connected; would throw if we weren't.
        qemu = QEMU.connection

        inferior = gdb.selected_inferior()
        if not inferior:
            error("tt: no inferior selected")
            return

        arch = inferior.architecture().name()

        if arch == "aarch64":
            log("tt: attempting to translate using CPU to verify walk")
            gdb.execut(f"oro tt at {virt:#x}")
        else:
            error("tt: unsupported architecture")


class TtCmdAt(gdb.Command):
    """
    Translate a virtual address to a physical address using the CPU's translation table
    functionality.

    Not available on all architectures.

    Usage: oro tt at <address>
    """

    def __init__(self):
        super(TtCmdAt, self).__init__("oro tt at", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro tt at")
            return

        try:
            virt = int(args[0], 0)
        except ValueError:
            gdb.execute("help oro tt at")
            return

        if virt < 0:
            gdb.execute("help oro tt at")
            return

        inferior = gdb.selected_inferior()
        if not inferior:
            error("tt: no inferior selected")
            return

        arch = inferior.architecture().name()

        if arch == "aarch64":
            # Attempt to lookup the `AT` stub.
            at_sym = SYMBOLS.get(SYM_AARCH64_ATS1E1R)
            if not at_sym:
                warn("tt: AT S1E1R stub not found; cannot perform CPU translation")
            (at_sym, _) = at_sym

            # If we're running and we can perform a manual translation, do so.
            frame = None
            try:
                frame = gdb.newest_frame()
                if frame.pc() == 0:
                    warn(
                        "tt: frame found, but is sitting at 0x0 (kernel is either not running or has crashed); cannot perform CPU translation"
                    )
                    return
            except gdb.error:
                warn(
                    "tt: no frame found (kernel isn't running); cannot perform CPU translation"
                )
                return

            original_pc = int(frame.pc())
            original_x0 = int(gdb.parse_and_eval("$x0"))
            gdb.parse_and_eval(f"$x0 = {virt:#x}")
            gdb.parse_and_eval(f"$pc = {at_sym:#x}")
            gdb.execute("stepi", to_string=True)
            translated = int(gdb.parse_and_eval("$PAR_EL1"))
            gdb.parse_and_eval(f"$x0 = {original_x0:#x}")
            gdb.parse_and_eval(f"$pc = {original_pc:#x}")

            aborted = (translated & 1) == 1
            if aborted:
                fst = (translated >> 1) & 0b111111
                ptw = (translated >> 8) & 1
                s = (translated >> 9) & 1

                fst = f"0b{fst:06b}"

                fst_reason = _FAULT_STATUSES.get(fst, "Unknown")
                ptw_reason = (
                    ""
                    if ptw == 0
                    else "(stage 2 fault during a stage 1 translation table walk)"
                )
                s_stage = 1 if s == 0 else 2

                warn(f"tt: PAR_EL1.F    = 1                   (translation aborted)")
                warn(f"tt: PAR_EL1.FST  = {fst}            ({fst_reason})")
                warn(f"tt: PAR_EL1.PTW  = {ptw}                   {ptw_reason}")
                warn(
                    f"tt: PAR_EL1.S    = {s}                   (fault occurred in stage {s_stage})"
                )
            else:
                sh = (translated >> 7) & 0b11
                ns = (translated >> 9) & 1
                pa = translated & 0xF_FFFF_FFFF_F000
                mair_attr = (translated >> 56) & 0xFF

                sh_reason = _SHAREABILITY[sh]

                log(f"tt: PAR_EL1.F    = 0                   (translation OK)")
                log(f"tt: PAR_EL1.PA   = 0x{pa:016x}")
                log(f"tt: PAR_EL1.SH   = {sh}                   ({sh_reason})")
                log(f"tt: PAR_EL1.NS   = {ns}")
                log(f"tt: PAR_EL1.ATTR = 0b{mair_attr:08b}          (MAIR value)")
        else:
            error("tt: unsupported architecture")


TtCmd()
TtCmdVirt()
TtCmdAt()
