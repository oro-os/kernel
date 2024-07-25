import gdb  # type: ignore
from ..log import log, error, warn
from ..arch import get_arch


class RegCmd(gdb.Command):
    """
    Inspect / decode a register value.

    Not all registers are supported, and each is architecture-specific.
    """

    def __init__(self):
        super(RegCmd, self).__init__("oro reg", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro reg")
            return

        reg = args[0].lower()
        arch = get_arch()

        if arch == "aarch64":
            if reg == "tcr_el1":
                return self._decode_tcr_el1()

        error(f"reg: register '{reg}' not supported for architecture '{arch}'")

    def _decode_tcr_el1(self):
        value = gdb.parse_and_eval("$TCR_EL1")
        if value.type.code == gdb.TYPE_CODE_VOID:
            error("reg: TCR_EL1 register not available (is the kernel running?)")
            return

        value = int(value)

        log(f"reg: TCR_EL1\t= 0x{value:016X}")

        mtx1 = (value >> 61) & 1
        mtx1_reason = (
            "no effect" if mtx1 == 0 else "[59:56] hold TT1 logical address tag"
        )
        log(f"reg: TCR_EL1.MTX1\t= {mtx1}\t\t\t({mtx1_reason})")

        mtx0 = (value >> 60) & 1
        mtx0_reason = (
            "no effect" if mtx0 == 0 else "[59:56] hold TT0 logical address tag"
        )
        log(f"reg: TCR_EL1.MTX0\t= {mtx0}\t\t\t({mtx0_reason})")

        ds = (value >> 59) & 1
        log(f"reg: TCR_EL1.DS\t= {ds}")

        tcma1 = (value >> 58) & 1
        tcma1_reason = (
            "no effect" if tcma1 == 0 else "EL1 accesses from EL1 and EL0 are unchecked"
        )
        log(f"reg: TCR_EL1.TCMA1\t= {tcma1}\t\t\t({tcma1_reason})")

        tcma0 = (value >> 57) & 1
        tcma0_reason = (
            "no effect" if tcma0 == 0 else "EL0 accesses from EL1 and EL0 are unchecked"
        )
        log(f"reg: TCR_EL1.TCMA0\t= {tcma0}\t\t\t({tcma0_reason})")

        e0pd1 = (value >> 56) & 1
        e0pd1_reason = (
            "EL0 translations of TT1 are allowed"
            if e0pd1 == 0
            else "EL0 translations of TT1 will fault"
        )
        log(f"reg: TCR_EL1.E0PD1\t= {e0pd1}\t\t\t({e0pd1_reason})")

        e0pd0 = (value >> 55) & 1
        e0pd0_reason = (
            "EL0 translations of TT0 are allowed"
            if e0pd0 == 0
            else "EL0 translations of TT0 will fault"
        )
        log(f"reg: TCR_EL1.E0PD0\t= {e0pd0}\t\t\t({e0pd0_reason})")

        nfd1 = (value >> 54) & 1
        log(f"reg: TCR_EL1.NFD1\t= {nfd1}")

        nfd0 = (value >> 53) & 1
        log(f"reg: TCR_EL1.NFD0\t= {nfd0}")

        tbid1 = (value >> 52) & 1
        tbid1_reason = (
            "TCR_EL1.TBI1 applies to instruction and data accesses"
            if tbid1 == 0
            else "TCR_EL1.TBI1 applies to data accesses only"
        )
        log(f"reg: TCR_EL1.TBID1\t= {tbid1}\t\t\t({tbid1_reason})")

        tbid0 = (value >> 51) & 1
        tbid0_reason = (
            "TCR_EL1.TBI0 applies to instruction and data accesses"
            if tbid0 == 0
            else "TCR_EL1.TBI0 applies to data accesses only"
        )
        log(f"reg: TCR_EL1.TBID0\t= {tbid0}\t\t\t({tbid0_reason})")

        hwu162 = (value >> 50) & 1
        log(f"reg: TCR_EL1.HWU162\t= {hwu162}")

        hwu161 = (value >> 49) & 1
        log(f"reg: TCR_EL1.HWU161\t= {hwu161}")

        hwu160 = (value >> 48) & 1
        log(f"reg: TCR_EL1.HWU160\t= {hwu160}")

        hwu159 = (value >> 47) & 1
        log(f"reg: TCR_EL1.HWU159\t= {hwu159}")

        hwu062 = (value >> 46) & 1
        log(f"reg: TCR_EL1.HWU062\t= {hwu062}")

        hwu061 = (value >> 45) & 1
        log(f"reg: TCR_EL1.HWU061\t= {hwu061}")

        hwu060 = (value >> 44) & 1
        log(f"reg: TCR_EL1.HWU060\t= {hwu060}")

        hwu059 = (value >> 43) & 1
        log(f"reg: TCR_EL1.HWU059\t= {hwu059}")

        hpd1 = (value >> 42) & 1
        hpd1_reason = "enabled" if hpd1 == 0 else "disabled"
        log(
            f"reg: TCR_EL1.HPD1\t= {hpd1}\t\t\t(TT1 hierarchical permissions are {hpd1_reason})"
        )

        hpd0 = (value >> 41) & 1
        hpd0_reason = "enabled" if hpd0 == 0 else "disabled"
        log(
            f"reg: TCR_EL1.HPD0\t= {hpd0}\t\t\t(TT0 hierarchical permissions are {hpd0_reason})"
        )

        ha = (value >> 39) & 1
        hd = (value >> 40) & 1

        hd_reason = (
            "disabled"
            if hd == 0
            else ("disabled (HA=0)" if ha == 0 else "enabled (HA=1)")
        )
        log(f"reg: TCR_EL1.HD\t= {hd}\t\t\t(dirty state is {hd_reason})")
        ha_reason = "disabled" if ha == 0 else "enabled"
        log(f"reg: TCR_EL1.HA\t= {ha}\t\t\t(access flag is {ha_reason})")

        tbi1 = (value >> 38) & 1
        tbi1_reason = "used" if tbi1 == 0 else "ignored"
        log(f"reg: TCR_EL1.TBI1\t= {tbi1}\t\t\t(TT1 top byte is {tbi1_reason})")

        tbi0 = (value >> 37) & 1
        tbi0_reason = "used" if tbi0 == 0 else "ignored"
        log(f"reg: TCR_EL1.TBI0\t= {tbi0}\t\t\t(TT0 top byte is {tbi0_reason})")

        asid_size = (value >> 36) & 1
        asid_size_reason = "8 bits" if asid_size == 0 else "16 bits"
        log(f"reg: TCR_EL1.AS\t= {asid_size}\t\t\t(ASID size is {asid_size_reason})")

        ips = (value >> 32) & 0b111
        ips_reason = [
            "32 bits, 4GB",
            "36 bits, 64GB",
            "40 bits, 1TB",
            "42 bits, 4TB",
            "44 bits, 16TB",
            "48 bits, 256TB",
            "52 bits, 4PB",
            "56 bits, 64PB (FEAT_D128, 64KiB granule only, otherwise reserved)",
        ][ips]
        log(f"reg: TCR_EL1.IPS\t= {ips}\t\t\t({ips_reason})")

        if ips == 0b111:
            warn(
                "reg: TCR_EL1.IPS might have an invalid (reserved) value, depending on granule size / FEAT_LPA2!"
            )

        tg1 = (value >> 30) & 0b11
        tg1_reason = [
            "invalid",
            "16KB",
            "4KB",
            "64KB",
        ][tg1]
        log(f"reg: TCR_EL1.TG1\t= {tg1}\t\t\t(TT1 granule size is {tg1_reason})")
        if tg1 == 0:
            warn("reg: TCR_EL1.TG1 has an invalid (reserved) value!")

        sh1 = (value >> 28) & 0b11
        sh1_reason = [
            "non-shareable",
            "invalid",
            "outer shareable",
            "inner shareable",
        ][sh1]
        log(f"reg: TCR_EL1.SH1\t= {sh1}\t\t\t(TT1 is {sh1_reason})")

        orgn1 = (value >> 26) & 0b11
        orgn1_reason = [
            "normal memory, outer non-cacheable",
            "normal memory, outer write-back read-allocate write-allocate cacheable",
            "normal memory, outer write-through read-allocate no write-allocate cacheable",
            "normal memory, outer write-back read-allocate no write-allocate cacheable",
        ][orgn1]
        log(
            f"reg: TCR_EL1.ORGN1\t= {orgn1}\t\t\t(TT1 outer cacheability is {orgn1_reason})"
        )

        irgn1 = (value >> 24) & 0b11
        irgn1_reason = [
            "normal memory, inner non-cacheable",
            "normal memory, inner write-back read-allocate write-allocate cacheable",
            "normal memory, inner write-through read-allocate no write-allocate cacheable",
            "normal memory, inner write-back read-allocate no write-allocate cacheable",
        ][irgn1]
        log(
            f"reg: TCR_EL1.IRGN1\t= {irgn1}\t\t\t(TT1 inner cacheability is {irgn1_reason})"
        )

        epd1 = (value >> 23) & 1
        epd1_reason = (
            "EL1 translations of TT1 are allowed"
            if epd1 == 0
            else "EL1 translations of TT1 will fault"
        )
        log(f"reg: TCR_EL1.EPD1\t= {epd1}\t\t\t({epd1_reason})")

        a1 = (value >> 22) & 1
        log(f"reg: TCR_EL1.A1\t= {a1}\t\t\t(TTBR{a1}_EL1.ASID defines the ASID)")

        t1sz = (value >> 16) & 63
        log(f"reg: TCR_EL1.T1SZ\t= {t1sz}\t\t\t(TT1 region size is {64 - t1sz} bits)")
        tt1_start = 0xFFFF_FFFF_FFFF_FFFF - pow(2, 64 - t1sz) + 1
        log(f"reg:             \t  0x{tt1_start:016X} - 0xFFFFFFFFFFFFFFFF")

        tg0 = (value >> 14) & 0b11
        tg0_reason = [
            "4KB",
            "64KB",
            "16KB",
            "invalid",
        ][tg0]

        log(f"reg: TCR_EL1.TG0\t= {tg0}\t\t\t(TT0 granule size is {tg0_reason})")
        if tg0 == 3:
            warn("reg: TCR_EL1.TG0 has an invalid (reserved) value!")

        sh0 = (value >> 12) & 0b11
        sh0_reason = [
            "non-shareable",
            "invalid",
            "outer shareable",
            "inner shareable",
        ][sh0]
        log(f"reg: TCR_EL1.SH0\t= {sh0}\t\t\t(TT0 is {sh0_reason})")

        orgn0 = (value >> 10) & 0b11
        orgn0_reason = [
            "normal memory, outer non-cacheable",
            "normal memory, outer write-back read-allocate write-allocate cacheable",
            "normal memory, outer write-through read-allocate no write-allocate cacheable",
            "normal memory, outer write-back read-allocate no write-allocate cacheable",
        ][orgn0]
        log(
            f"reg: TCR_EL1.ORGN0\t= {orgn0}\t\t\t(TT0 outer cacheability is {orgn0_reason})"
        )

        irgn0 = (value >> 8) & 0b11
        irgn0_reason = [
            "normal memory, inner non-cacheable",
            "normal memory, inner write-back read-allocate write-allocate cacheable",
            "normal memory, inner write-through read-allocate no write-allocate cacheable",
            "normal memory, inner write-back read-allocate no write-allocate cacheable",
        ][irgn0]
        log(
            f"reg: TCR_EL1.IRGN0\t= {irgn0}\t\t\t(TT0 inner cacheability is {irgn0_reason})"
        )

        epd0 = (value >> 7) & 1
        epd0_reason = (
            "EL1 translations of TT0 are allowed"
            if epd0 == 0
            else "EL1 translations of TT0 will fault"
        )
        log(f"reg: TCR_EL1.EPD0\t= {epd0}\t\t\t({epd0_reason})")

        t0sz = value & 63
        log(f"reg: TCR_EL1.T0SZ\t= {t0sz}\t\t\t(TT0 region size is {64 - t0sz} bits)")
        tt0_end = pow(2, 64 - t0sz) - 1
        log(f"reg:             \t  0x0000000000000000 - 0x{tt0_end:016X}")


RegCmd()
