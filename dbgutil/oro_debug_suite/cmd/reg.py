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

        if len(args) < 1 or len(args) > 2:
            gdb.execute("help oro reg")
            return

        reg = args[0].lower()
        arch = get_arch()
        value = int(args[1], 0) if len(args) == 2 else None

        if arch == "aarch64":
            if reg == "tcr_el1":
                return self._decode_tcr_el1(value)
        elif arch == "i386:x86-64":
            if reg == "cr0":
                return self._decode_cr0(value)
            if reg == "eflags":
                return self._decode_eflags(value)

        error(f"reg: register '{reg}' not supported for architecture '{arch}'")

    def _decode_tcr_el1(self, value=None):
        if value is None:
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

    def _decode_cr0(self, value=None):
        if value is None:
            value = gdb.parse_and_eval("$cr0")
            if value.type.code == gdb.TYPE_CODE_VOID:
                error("reg: CR0 register not available (is the kernel running?)")
                return

            value = int(value)

        log(f"reg: CR0\t= 0x{value:016X}")
        log(f"reg:    \t= 0b{(value & 0xFFFF_FFFF):032b}")

        # fmt: off
        pe = (value >> 0) & 1
        mp = (value >> 1) & 1
        em = (value >> 2) & 1
        ts = (value >> 3) & 1
        et = (value >> 4) & 1
        ne = (value >> 5) & 1
        res6 = (value >> 6) & 0x3FF
        wp = (value >> 16) & 1
        res17 = (value >> 17) & 1
        am = (value >> 18) & 1
        res19 = (value >> 19) & 0x3FF
        nw = (value >> 29) & 1
        cd = (value >> 30) & 1
        pg = (value >> 31) & 1
        res32 = (value >> 32) & 0xFFFF_FFFF

        log(f"reg:    .PE\t= {pe} ({'protected mode' if pe == 1 else 'real mode'})")
        log(f"reg:    .MP\t= {mp} ({'monitor coprocessor' if mp == 1 else 'no monitor coprocessor'})")
        log(f"reg:    .EM\t= {em} ({'emulation' if em == 1 else 'no emulation'})")
        log(f"reg:    .TS\t= {ts} ({'task switched' if ts == 1 else 'task not switched'})")
        log(f"reg:    .ET\t= {et} ({'external math processor is 80387' if et == 1 else 'external math processor is 80287'})")
        log(f"reg:    .NE\t= {ne} ({'numeric error' if ne == 1 else 'no numeric error'})")
        log(f"reg:    .WP\t= {wp} ({'supervisor write protect on RO user pages' if wp == 1 else 'supervisor can write to RO user pages'})")
        log(f"reg:    .AM\t= {am} ({'alignment mask' if am == 1 else 'no alignment mask'})")
        if am:
            warn("reg: Reminder that CR0.AM has no effect in rings 0, 1 or 2.")
        log(f"reg:    .NW\t= {nw} ({'write-through' if nw == 1 else 'write-back'})")
        log(f"reg:    .CD\t= {cd} ({'cache disable' if cd == 1 else 'cache enable'})")
        warn(f"reg: Reminder that CR0.CD is updated for all logical core CR0 registers within the same physical core.")
        log(f"reg:    .PG\t= {pg} ({'paging enabled' if pg == 1 else 'paging disabled'})")
        log(f"reg:    .reserved[16:6]\t= 0b{res6:010b}")
        log(f"reg:    .reserved[17]\t= 0b{res17:01b}")
        log(f"reg:    .reserved[28:19]\t= 0b{res19:010b}")
        log(f"reg:    .reserved[63:32]\t= 0x{res32:08X}")
        # fmt: on

    def _decode_eflags(self, value=None):
        if value is None:
            value = gdb.parse_and_eval("$eflags")
            if value.type.code == gdb.TYPE_CODE_VOID:
                error("reg: EFLAGS register not available (is the kernel running?)")
                return

            value = int(value)

        log(f"reg: EFLAGS\t= 0x{value:016X}")
        log(f"reg:       \t= 0b{(value & 0xFFFF_FFFF):032b}")

        # fmt: off
        cf = (value >> 0) & 1
        res1 = (value >> 1) & 1
        pf = (value >> 2) & 1
        res3 = (value >> 3) & 1
        af = (value >> 4) & 1
        res5 = (value >> 5) & 1
        zf = (value >> 6) & 1
        sf = (value >> 7) & 1
        tf = (value >> 8) & 1
        _if = (value >> 9) & 1
        df = (value >> 10) & 1
        of = (value >> 11) & 1
        iopl = (value >> 12) & 0b11
        nt = (value >> 14) & 1
        res15 = (value >> 15) & 1
        rf = (value >> 16) & 1
        vm = (value >> 17) & 1
        ac = (value >> 18) & 1
        vif = (value >> 19) & 1
        vip = (value >> 20) & 1
        id = (value >> 21) & 1
        res22 = (value >> 22) & 0x3FF
        res32 = (value >> 32) & 0xFFFF_FFFF

        log(f"reg:       .CF\t= {cf} ({'carry' if cf == 1 else 'no carry'})")
        if res1 == 0:
            warn("reg: EFLAGS[1] is reserved and should be 1.")
        log(f"reg:       .PF\t= {pf} ({'parity' if pf == 1 else 'no parity'})")
        if res3 == 1:
            warn("reg: EFLAGS[3] is reserved and should be 0.")
        log(f"reg:       .AF\t= {af} ({'auxiliary carry' if af == 1 else 'no auxiliary carry'})")
        if res5 == 1:
            warn("reg: EFLAGS[5] is reserved and should be 0.")
        log(f"reg:       .ZF\t= {zf} ({'zero' if zf == 1 else 'non-zero'})")
        log(f"reg:       .SF\t= {sf} ({'negative' if sf == 1 else 'non-negative'})")
        log(f"reg:       .TF\t= {tf} ({'trap' if tf == 1 else 'no trap'})")
        log(f"reg:       .IF\t= {_if} ({'interrupts enabled' if _if == 1 else 'interrupts disabled'})")
        log(f"reg:       .DF\t= {df} ({'direction' if df == 1 else 'no direction'})")
        log(f"reg:       .OF\t= {of} ({'overflow' if of == 1 else 'no overflow'})")
        log(f"reg:       .IOPL\t= {iopl} (ring {iopl})")
        log(f"reg:       .NT\t= {nt} ({'nested task' if nt == 1 else 'no nested task'})")
        if res15 == 1:
            warn("reg: EFLAGS[15] is reserved and should be 0.")
        log(f"reg:       .RF\t= {rf} ({'resume' if rf == 1 else 'no resume'})")
        log(f"reg:       .VM\t= {vm} ({'virtual 8086 mode' if vm == 1 else 'no virtual 8086 mode'})")
        log(f"reg:       .AC\t= {ac} ({'alignment check' if ac == 1 else 'no alignment check'})")
        log(f"reg:       .VIF\t= {vif} ({'virtual interrupt flag' if vif == 1 else 'no virtual interrupt flag'})")
        log(f"reg:       .VIP\t= {vip} ({'virtual interrupt pending' if vip == 1 else 'no virtual interrupt pending'})")
        log(f"reg:       .ID\t= {id} ({'identification' if id == 1 else 'no identification'})")
        # fmt: on


RegCmd()
