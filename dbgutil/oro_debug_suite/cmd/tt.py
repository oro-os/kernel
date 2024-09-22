import struct
import gdb  # type: ignore
from ..service import QEMU, SYMBOLS
from ..service.autosym import SYM_AARCH64_ATS1E1R
from ..log import error, log, warn
from ..arch import get_arch

_AARCH64_AT_FAULT_STATUSES = {
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

_AARCH64_SHAREABILITY = [
    "non-shareable",
    "reserved (invalid shareability)",
    "outer shareable / device",
    "inner shareable",
]


class TtCmd(gdb.Command):
    """
    Memory translation utilities.
    """

    def __init__(self):
        super(TtCmd, self).__init__("oro tt", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, from_tty=False):
        gdb.execute("help oro tt")


class TranslationAbort(Exception):
    pass


class TtCmdVirt(gdb.Command):
    """
    Translate a virtual address to a physical address.

    Usage: oro tt virt <address>
    """

    def __init__(self):
        super(TtCmdVirt, self).__init__("oro tt virt", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro tt virt")
            return

        try:
            virt = int(gdb.parse_and_eval(args[0])) & 0xFFFF_FFFF_FFFF_FFFF
        except ValueError as e:
            error(f"tt: failed to parse expression: {str(e)}")
            gdb.execute("help oro tt virt")
            return

        if virt < 0:
            gdb.execute("help oro tt virt")
            return

        qemu = QEMU.session

        arch = get_arch()

        if arch == "aarch64":
            # TODO(qix-): Make sure we're not using 128 bit descriptors
            # TODO(qix-): Determine endianness of the target

            tcr_el1 = int(gdb.parse_and_eval("$TCR_EL1"))
            if tcr_el1 == 0:
                error(
                    "tt: TCR_EL1=0; cannot perform translation (is the kernel running?)"
                )
                return

            # Is DS set?
            ds = (tcr_el1 >> 59) & 1
            if ds == 1:
                error("tt: TCR_EL1.DS=1, which is not supported by the translator")
                return
            log("tt: TCR_EL1.DS\t= 0\t\t\t(ok)")

            # Get the tt ranges
            t0sz = tcr_el1 & 63
            t1sz = (tcr_el1 >> 16) & 63
            log(f"tt: TCR_EL1.T0SZ\t= {t0sz}")
            log(f"tt: TCR_EL1.T1SZ\t= {t1sz}")

            # (ref RYYVYV)
            tt0_start = 0x0
            tt0_end = pow(2, 64 - t0sz) - 1
            tt1_start = 0xFFFF_FFFF_FFFF_FFFF - pow(2, 64 - t0sz) + 1
            tt1_end = 0xFFFF_FFFF_FFFF_FFFF

            log(f"tt: TT0 range\t= 0x{tt0_start:016x} - 0x{tt0_end:016x}")
            log(f"tt: TT1 range\t= 0x{tt1_start:016x} - 0x{tt1_end:016x}")

            if virt >= tt0_start and virt <= tt0_end:
                tt_range = 0
            elif virt >= tt1_start and virt <= tt1_end:
                tt_range = 1
            else:
                error(f"tt: virtual address 0x{virt:016x} is not in any TT range")
                return

            log(f"tt: VA.RANGE\t= TT{tt_range}")

            # Make sure it matches the TT spec (bit[55])
            # (ref RVZCSR)
            tt55_bit = (virt >> 55) & 1
            tt55_range = 0 if tt_range == 0 else 1  # Just being extra explicit here.

            log(f"tt: VA[55]\t\t= {tt55_bit}")
            log(f"tt: VA.TT\t\t= {tt55_range}")

            if tt55_bit != tt55_range:
                error("tt: VA[55] does not match the TT range")
                warn("tt: this is probably a bug in the translator")
                return

            # Get the granule size
            if tt_range == 0:
                granule_bits = (tcr_el1 >> 14) & 0b11
                if granule_bits == 0b00:
                    granule_size = 4
                elif granule_bits == 0b10:
                    granule_size = 16
                elif granule_bits == 0b01:
                    granule_size = 64
                else:
                    error(
                        f"tt: TCR_EL1.TG0 reports an invalid granule size 0b({granule_bits:02b})"
                    )
                    return
            else:
                granule_bits = (tcr_el1 >> 30) & 0b11
                if granule_bits == 0b10:
                    granule_size = 4
                elif granule_bits == 0b01:
                    granule_size = 16
                elif granule_bits == 0b11:
                    granule_size = 64
                else:
                    error(
                        f"tt: TCR_EL1.TG1 reports an invalid granule size 0b({granule_bits:02b})"
                    )
                    return

            log(f"tt: TCR_EL1.TG{tt_range}\t= {granule_size}KiB")

            if granule_size != 4:
                error(
                    f"tt: TCR_EL1.TG{tt_range}={granule_size}KiB is not supported by the translator (only 4KiB is)"
                )
                return

            # Load the TTBR value
            ttbr = int(gdb.parse_and_eval(f"$TTBR{tt_range}_EL1"))
            log(f"tt: TTBR{tt_range}_EL1\t= 0x{ttbr:016x}")

            # Get the ASID selection (A1)
            asid_select = (tcr_el1 >> 22) & 1
            asid_select = (
                0 if asid_select == 0 else 1
            )  # Just being extra explicit here.
            log(f"tt: TCR_EL1.A1\t= TT{asid_select}")

            # Get the ASID size
            asid_size = (tcr_el1 >> 36) & 1
            asid_size = 8 if asid_size == 0 else 16
            log(f"tt: TCR_EL1.AS\t= {asid_size} bits")

            if tt_range == asid_select:
                # Get the ASID
                asid = (ttbr >> 48) & ((1 << asid_size) - 1)
                log(f"tt: TTBR{tt_range}_EL1.ASID\t= 0x{asid:0{asid_size//4}x}")

            # Masks off all ASID bits as well as the CPL bits (bit[0])
            ttbr_pa = ttbr & (((1 << (64 - asid_size)) - 1) ^ 1)
            log(f"tt: TTBR{tt_range}_EL1.PA\t= 0x{ttbr_pa:016x}")

            # Adjust the address to be relative to the TTBR start range
            virt_rel = virt - (tt0_start if tt_range == 0 else tt1_start)
            log(f"tt: VA.REL\t\t= 0x{virt_rel:016x}")

            try:
                prefix = " " * 7

                def print_table_entry(raw):
                    assert (
                        raw >> 1
                    ) & 1 == 1, (
                        "tried to print table entry but entry is not a table entry"
                    )

                    addr = raw & 0xFFFF_FFFF_F000
                    ns = (raw >> 63) & 1
                    ap = (raw >> 61) & 0b11
                    xn = (raw >> 60) & 1
                    xn2 = (raw >> 59) & 1
                    protected = (raw >> 52) & 1
                    access = (raw >> 10) & 1

                    log(f"tt: {prefix}.ADDR\t= 0x{addr:016x}")
                    log(f"tt: {prefix}.NS\t= {ns}")
                    log(f"tt: {prefix}.AP\t= 0b{ap:02b}")
                    log(f"tt: {prefix}.XN\t= {xn}")
                    log(f"tt: {prefix}.XN2\t= {xn2}")
                    log(f"tt: {prefix}.PROT\t= {protected}")
                    log(f"tt: {prefix}.AF\t= {access}")

                    return addr

                # Read the L0 page table entry
                l0_index = (virt_rel >> 39) & 511
                l0_index_s = f"{l0_index:03}"
                log(f"tt: L0.IDX\t\t= {l0_index}")
                l0_entry_pa = ttbr_pa + (l0_index * 8)
                log(f"tt: L0[{l0_index_s}].PA\t= 0x{l0_entry_pa:016x}")
                l0_entry = qemu.read_physical(l0_entry_pa, 8)
                (l0_entry,) = struct.unpack("<Q", l0_entry)
                log(f"tt: L0[{l0_index_s}]\t\t= 0x{l0_entry:016x}")

                valid = l0_entry & 1
                log(
                    f"tt: {prefix}.V\t= {valid}\t\t\t({'valid' if valid == 1 else 'invalid'})"
                )
                if valid == 0:
                    warn(f"tt: L0 entry is invalid; translation aborted")
                    raise TranslationAbort()

                table = (l0_entry >> 1) & 1
                log(f"tt: {prefix}.T\t= {table}\t\t\t({'table' if table else 'block'})")
                if not table:
                    error(
                        f"tt: L0 entry is a block; the translator only supports 4-level translations"
                    )
                    return

                l1_pa = print_table_entry(l0_entry)

                # Read the L1 page table entry
                l1_index = (virt_rel >> 30) & 511
                l1_index_s = f"{l1_index:03}"
                log(f"tt: L1.IDX\t\t= {l1_index}")
                l1_entry_pa = l1_pa + (l1_index * 8)
                log(f"tt: L1[{l1_index_s}].PA\t= 0x{l1_entry_pa:016x}")
                l1_entry = qemu.read_physical(l1_entry_pa, 8)
                (l1_entry,) = struct.unpack("<Q", l1_entry)
                log(f"tt: L1[{l1_index_s}]\t\t= 0x{l1_entry:016x}")

                valid = l1_entry & 1
                log(
                    f"tt: {prefix}.V\t= {valid}\t\t\t({'valid' if valid == 1 else 'invalid'})"
                )
                if valid == 0:
                    warn(f"tt: L1 entry is invalid; translation aborted")
                    raise TranslationAbort()

                table = (l1_entry >> 1) & 1
                log(f"tt: {prefix}.T\t= {table}\t\t\t({'table' if table else 'block'})")
                if not table:
                    error(
                        f"tt: L1 entry is a block; the translator only supports 4-level translations"
                    )
                    return

                l2_pa = print_table_entry(l1_entry)

                # Read the L2 page table entry
                l2_index = (virt_rel >> 21) & 511
                l2_index_s = f"{l2_index:03}"
                log(f"tt: L2.IDX\t\t= {l2_index}")
                l2_entry_pa = l2_pa + (l2_index * 8)
                log(f"tt: L2[{l2_index_s}].PA\t= 0x{l2_entry_pa:016x}")
                l2_entry = qemu.read_physical(l2_entry_pa, 8)
                (l2_entry,) = struct.unpack("<Q", l2_entry)
                log(f"tt: L2[{l2_index_s}]\t\t= 0x{l2_entry:016x}")

                valid = l2_entry & 1
                log(
                    f"tt: {prefix}.V\t= {valid}\t\t\t({'valid' if valid == 1 else 'invalid'})"
                )
                if valid == 0:
                    warn(f"tt: L2 entry is invalid; translation aborted")
                    raise TranslationAbort()

                table = (l2_entry >> 1) & 1
                log(f"tt: {prefix}.T\t= {table}\t\t\t({'table' if table else 'block'})")
                if not table:
                    error(
                        f"tt: L2 entry is a block; the translator only supports 4-level translations"
                    )
                    return

                l3_pa = print_table_entry(l2_entry)

                # Read the L3 page table entry (page)
                l3_index = (virt_rel >> 12) & 511
                l3_index_s = f"{l3_index:03}"
                log(f"tt: L3.IDX\t\t= {l3_index}")
                l3_entry_pa = l3_pa + (l3_index * 8)
                log(f"tt: L3[{l3_index_s}].PA\t= 0x{l3_entry_pa:016x}")
                l3_entry = qemu.read_physical(l3_entry_pa, 8)
                (l3_entry,) = struct.unpack("<Q", l3_entry)
                log(f"tt: L3[{l3_index_s}]\t\t= 0x{l3_entry:016x}")

                valid = l3_entry & 0b11
                log(
                    f"tt: {prefix}.V\t= 0b{valid:02b}\t\t\t({'valid' if valid == 0b11 else 'invalid'})"
                )
                if valid != 0b11:
                    warn(f"tt: L3 entry is invalid; translation aborted")
                    raise TranslationAbort()

                out_addr = l3_entry & 0xFFFF_FFFF_F000
                mecid = (l3_entry >> 63) & 1
                pbha31 = (l3_entry >> 60) & 0b111
                pbha0 = (l3_entry >> 59) & 1
                software = (l3_entry >> 56) & 0b111
                xn = (l3_entry >> 54) & 1
                xn2 = (l3_entry >> 53) & 1
                contiguous = (l3_entry >> 52) & 1
                dirty = (l3_entry >> 51) & 1
                guarded = (l3_entry >> 50) & 1
                ng = (l3_entry >> 11) & 1
                access_flag = (l3_entry >> 10) & 1
                shareability = (l3_entry >> 8) & 0b11
                ap = (l3_entry >> 6) & 0b11
                ns = (l3_entry >> 5) & 1
                mair_idx = (l3_entry >> 2) & 0b111

                log(f"tt: {prefix}.PA\t= 0x{out_addr:016x}")
                log(f"tt: {prefix}.MECID\t= {mecid}")
                log(f"tt: {prefix}.PBHA31\t= 0b{pbha31:03b}")
                log(f"tt: {prefix}.PBHA0\t= {pbha0}")
                log(f"tt: {prefix}.SW\t= 0b{software:03b}")
                log(f"tt: {prefix}.XN\t= {xn}")
                log(f"tt: {prefix}.XN2\t= {xn2}")
                log(
                    f"tt: {prefix}.CNTG\t= {contiguous}\t\t\t(`PROT` if TCR2_EL1.PnCH=1)"
                )
                log(
                    f"tt: {prefix}.DRT\t= {dirty}\t\t\t(PIIndex[1] if indirect permissions are enabled)"
                )
                log(
                    f"tt: {prefix}.GRD\t= {guarded}\t\t\t(only if FEAT_BTI is implemented)"
                )
                log(
                    f"tt: {prefix}.NG\t= {ng}\t\t\t(only if two privilege levels are used)"
                )
                log(f"tt: {prefix}.AF\t= {access_flag}")
                if access_flag == 0:
                    warn(
                        "tt: access flag is not set; CPU will most likely fault unless hardware A/D flags are enabled"
                    )
                log(
                    f"tt: {prefix}.SH\t= 0b{shareability:02b}\t\t\t({_AARCH64_SHAREABILITY[shareability]})"
                )
                log(
                    f"tt: {prefix}.AP\t= 0b{ap:02b}\t\t\t(only when indirect permissions are disabled)"
                )
                log(f"tt: {prefix}.NS\t= {ns}\t\t\t(only from secure state)")
                log(f"tt: {prefix}.MAIR\t= {mair_idx}")
            except TranslationAbort:
                pass

            log("tt:")
            log("tt: verifying manual walk with CPU translation...")
            cpu_translated = CMD_AT.invoke(arg)

            if cpu_translated is not None:
                log(f"tt:")
                if cpu_translated == out_addr:
                    log(f"tt: CPU translation matches manual walk - OK!")
                else:
                    warn(f"tt: CPU translation does not match manual walk")
        elif arch == "i386:x86-64":
            cr0 = int(gdb.parse_and_eval("$cr0"))
            cr4 = int(gdb.parse_and_eval("$cr4"))
            la57 = (cr4 & (1 << 12)) != 0
            pge = (cr4 & (1 << 7)) != 0
            pae = (cr4 & (1 << 5)) != 0
            pe = (cr0 & 1) != 0
            pg = (cr0 & (1 << 31)) != 0

            warn("tt: cannot check CPUID; PSE and 1G page support cannot be verified")
            warn("tt: cannot check EFER.LME; assuming long mode is enabled")

            if not pae:
                log("tt: CR4.PAE=0; translation in this mode is not supported")
                return
            if not pe:
                log("tt: CR0.PE=0; real mode is not supported")
                return
            if not pg:
                warn("tt: CR0.PG=0; paging is not enabled!")

            current_pte_phys = int(gdb.parse_and_eval("$cr3"))

            log(f"tt: CR3=0x{current_pte_phys:016x}")
            log(
                f"tt: CR4.LA57={la57} (5-level paging is {'enabled' if la57 else 'disabled'})"
            )

            def print_pte(pte, level):
                log(f"tt: L{level}                = 0x{pte:016x}")
                p = pte & 1
                log(
                    f"tt:   .P              = {p} ({'present' if p else 'not present'})"
                )
                if not p:
                    error("tt: entry not present")
                    return None, False, False, None
                rw = (pte >> 1) & 1
                us = (pte >> 2) & 1
                pwt = (pte >> 3) & 1
                pcd = (pte >> 4) & 1
                a = (pte >> 5) & 1
                d_or_avl6 = (pte >> 6) & 1
                ps = (pte >> 7) & 1

                log(
                    f"tt:   .RW             = {rw} ({'read/write' if rw else 'read-only'})"
                )
                log(f"tt:   .US             = {us} ({'user' if us else 'supervisor'})")
                log(
                    f"tt:   .PWT            = {pwt} ({'write-through' if pwt else 'write-back'})"
                )
                log(
                    f"tt:   .PCD            = {pcd} ({'cacheable' if pcd else 'uncacheable'})"
                )
                log(
                    f"tt:   .A              = {a} ({'accessed' if a else 'not accessed'})"
                )
                if level == 1 or ps:
                    log(
                        f"tt:   .D              = {d_or_avl6} ({'dirty' if d_or_avl6 else 'not dirty'})"
                    )
                    if level > 3:
                        error(
                            f"tt:   .PS             = invalid (got 1; reserved, must be 0)"
                        )
                        return None, False, False, None
                    if level > 1:
                        log(
                            f"tt:   .PS             = 1 ({'page' if level == 1 else 'huge page'})"
                        )
                        warn("tt: continuing assuming huge pages are supported by CPU")
                    else:
                        log(f"tt:   .PAT            = {1 if ps else 0}")

                    g = (pte >> 8) & 1
                    avl9 = (pte >> 9) & 0b111
                    if level == 1:
                        pat = None
                        phys = pte & 0x000F_FFFF_FFFF_F000
                        reserved = None
                        mask = (1 << 12) - 1
                    elif level == 2:
                        pat = (pte >> 12) & 1
                        phys = pte & 0x000F_FFFF_FFF0_0000
                        reserved = (20, 13, pte & 0x1F_E000)
                        mask = (1 << 21) - 1
                    elif level == 3:
                        pat = (pte >> 12) & 1
                        phys = pte & 0x000F_FFFF_C000_0000
                        reserved = (29, 13, pte & 0x3FFF_E000)
                        mask = (1 << 30) - 1

                    avl52 = (pte >> 52) & 0x7F
                    pk = (pte >> 59) & 0xF
                    xd = (pte >> 63) & 1

                    log(
                        f"tt:   .G              = {g} ({'global' if g else 'not global'})"
                    )
                    if not pge:
                        warn(
                            f"tt: PTE.G=1 but CR4.PGE=0; global pages are not supported"
                        )
                        warn(
                            f"tt: (this may cause a #GP fault upon next page table walk)"
                        )

                    log(f"tt:   .AVL[11:9]      = 0b{avl9:03b} ({avl9})")

                    if pat is not None:
                        log(f"tt:   .PAT            = {1 if pat else 0}")
                    if reserved is not None:
                        (hi, lo, bits) = reserved
                        if bits != 0:
                            error(
                                f"tt:   .RSV[{hi}:{lo}]     = 0b{bin(bits)} (reserved, should be 0)"
                            )

                    log(f"tt:   .PHYS           = 0x{phys:016X}")
                    log(f"tt:   .AVL[58:52]     = 0b{avl52:07b} ({avl52})")
                    log(f"tt:   .PK             = 0x{pk:01X} ({pk})")
                    log(
                        f"tt:   .XD             = {xd} ({'execute disable' if xd else 'executable'})"
                    )

                    return phys, False, True, mask
                else:
                    log(f"tt:   .AVL[6]         = 0b{d_or_avl6:01b}")
                    log(f"tt:   .PS             = 0 (table entry)")

                    avl8 = (pte >> 8) & 0xF
                    phys = pte & 0x000F_FFFF_FFFF_F000
                    avl52 = (pte >> 52) & 0x7FF
                    xd = (pte >> 63) & 1

                    log(f"tt:   .AVL[11:8]      = 0b{avl8:04b} ({avl8})")
                    log(f"tt:   .PHYS           = 0x{phys:016x}")
                    log(f"tt:   .AVL[62:52]     = 0b{avl52:011b} ({avl52})")
                    log(
                        f"tt:   .XD             = {xd} ({'execute disable' if xd else 'executable'})"
                    )

                    return phys, True, True, None

            ok = False
            level = 5 if la57 else 4
            for level in range(level, 0, -1):
                idx = (virt >> (12 + 9 * (level - 1))) & 0x1FF
                log(f"tt: L{level} index: {idx}")
                pte = qemu.read_physical(current_pte_phys + idx * 8, 8)
                pte = struct.unpack("<Q", pte)[0]
                current_pte_phys, cont, ok, mask = print_pte(pte, level)
                if not cont:
                    break

            if ok:
                current_pte_phys |= virt & mask
                log(f"tt: physical address: 0x{current_pte_phys:016x}")

                log("tt: verifying manual walk with QEMU translation...")
                qemu_translated = qemu.gva2gpa(virt)
                if qemu_translated is not None:
                    log(f"tt: QEMU translation: 0x{qemu_translated:016x}")
                    if qemu_translated == current_pte_phys:
                        log("tt: QEMU translation matches manual walk - OK!")
                    else:
                        warn("tt: QEMU translation does not match manual walk")
                else:
                    error("tt: translation failed")

                if not from_tty:
                    return current_pte_phys
            else:
                error("tt: translation failed")
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

    def invoke(self, arg, from_tty=False):
        args = gdb.string_to_argv(arg)

        if len(args) != 1:
            gdb.execute("help oro tt at")
            return

        try:
            virt = int(gdb.parse_and_eval(args[0])) & 0xFFFF_FFFF_FFFF_FFFF
        except ValueError as e:
            error(f"tt: failed to parse expression: {str(e)}")
            gdb.execute("help oro tt at")
            return

        if virt < 0:
            gdb.execute("help oro tt at")
            return

        arch = get_arch()

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

            original_par_el1 = int(gdb.parse_and_eval("$PAR_EL1"))
            gdb.parse_and_eval("$PAR_EL1 = 0")
            original_pc = int(frame.pc())
            original_x0 = int(gdb.parse_and_eval("$x0"))
            gdb.parse_and_eval(f"$x0 = {virt:#x}")
            gdb.parse_and_eval(f"$pc = {at_sym:#x}")
            gdb.execute("stepi", to_string=True)
            translated = int(gdb.parse_and_eval("$PAR_EL1"))
            gdb.parse_and_eval(f"$x0 = {original_x0:#x}")
            gdb.parse_and_eval(f"$pc = {original_pc:#x}")
            gdb.parse_and_eval(f"$PAR_EL1 = {original_par_el1:#x}")

            if translated == 0:
                error("tt: translation failed (PAR_EL1=0); execution might have failed")
                warn(
                    "tt: PAR_EL1 wasn't set during the translation; the kernel state may not have properly been restored!"
                )
                warn(
                    "tt: check the logs and a \x1b[1mbt\x1b[m to double check, and make sure x0 wasn't clobbered!"
                )
                return

            aborted = (translated & 1) == 1
            if aborted:
                fst = (translated >> 1) & 0b111111
                ptw = (translated >> 8) & 1
                s = (translated >> 9) & 1

                fst = f"0b{fst:06b}"

                fst_reason = _AARCH64_AT_FAULT_STATUSES.get(fst, "Unknown")
                ptw_reason = (
                    ""
                    if ptw == 0
                    else "(stage 2 fault during a stage 1 translation table walk)"
                )
                s_stage = 1 if s == 0 else 2

                warn(f"tt: PAR_EL1.F\t= 1\t\t\t(translation aborted)")
                warn(f"tt: PAR_EL1.FST\t= {fst}\t\t({fst_reason})")
                warn(f"tt: PAR_EL1.PTW\t= {ptw}\t\t\t{ptw_reason}")
                warn(f"tt: PAR_EL1.S\t= {s}\t\t\t(fault occurred in stage {s_stage})")
            else:
                sh = (translated >> 7) & 0b11
                ns = (translated >> 9) & 1
                pa = translated & 0xF_FFFF_FFFF_F000
                mair_attr = (translated >> 56) & 0xFF

                sh_reason = _AARCH64_SHAREABILITY[sh]

                log(f"tt: PAR_EL1.F\t= 0\t\t\t(translation OK)")
                log(f"tt: PAR_EL1.PA\t= 0x{pa:016x}")
                log(f"tt: PAR_EL1.SH\t= {sh}\t\t\t({sh_reason})")
                log(f"tt: PAR_EL1.NS\t= {ns}")
                log(f"tt: PAR_EL1.ATTR\t= 0b{mair_attr:08b}\t\t(MAIR value)")

                if not from_tty:
                    return pa
        elif arch == "i386:x86-64":
            # Best we can do is to use QEMU's gva2gpa command.
            qemu = QEMU.session
            translated = qemu.gva2gpa(virt)
            if translated is None:
                error(
                    "tt: note: page tables may be valid, but gva2gpa will not succeed"
                )
                error(
                    "tt: note: if the translated physical address isn't a real address"
                )
                error("tt: translation failed")
                return
            log(f"tt: translation OK: 0x{translated:016x}")
            if not from_tty:
                return translated
        else:
            error(f"tt: unsupported architecture '{arch}'")


TtCmd()
TtCmdVirt()
CMD_AT = TtCmdAt()
