import gdb  # type: ignore
import os
from os import path
from ..log import log, error
from .. import gdb_util
import subprocess
from ..service import QEMU


class BootCmd(gdb.Command):
    """
    Convenience commands for booting the kernel via QEMU.
    """

    def __init__(self):
        super(BootCmd, self).__init__("oro boot", gdb.COMMAND_USER, prefix=True)

    def invoke(self, arg, _from_tty=False):
        gdb.execute("help oro boot")


class BootCmdLimine(gdb.Command):
    """
    Boot the kernel under QEMU using the Limine bootloader.

    Usage:
        oro boot limine [-sC] [-n <num_cores>]

    Options:
        -s, --switch       Switch to the Limine executable before booting.
        -n, --num_cores    Specify the number of CPU cores to emulate (default: 1).
        -C, --no-continue  Do not automatically continue execution after booting.
    """

    def __init__(self):
        super(BootCmdLimine, self).__init__("oro boot limine", gdb.COMMAND_USER)

    def invoke(self, arg, _from_tty=False):
        import shutil
        from ..bootstrap import (
            fetch_limine,
            get_site_packages_dir,
            check_bin_dep,
            check_bin_deps,
        )

        self.dont_repeat()

        args = gdb.string_to_argv(arg)
        rest_args = []

        switch = False
        num_cores = 2
        auto_continue = True

        argi = 0
        while argi < len(args):
            arg = args[argi]

            if arg in ["--switch", "-s"]:
                switch = True
            elif arg in ["--num_cores", "-n"]:
                if argi + 1 >= len(args):
                    error("missing argument for --num_cores")
                    return
                try:
                    num_cores = int(args[argi + 1])
                except ValueError:
                    error("invalid argument for --num_cores")
                    return

                if num_cores < 1:
                    error("invalid argument for --num_cores: must be greater than 0")
                    return

                argi += 1
            elif arg in ["--no-continue", "-C"]:
                auto_continue = False
            elif arg == "--":
                rest_args = args[argi + 1 :]
                break
            else:
                error(f"unknown argument: {arg}")
                return

            argi += 1

        # Fetch Limine if it doesn't exist
        limine_dir = fetch_limine()

        # Do we have everything we need to make the ISO?
        if not check_bin_deps("xorriso", "git", "make"):
            error(
                "missing required PATH utilities to build Limine ISO; install them and try again"
            )
            return

        # Are we currently the kernel file?
        current_progspace = gdb.current_progspace()
        if not current_progspace:
            error("no current progspace; cannot determine current file")
            return

        kernel_path = current_progspace.filename
        kernel_basename = path.basename(kernel_path)
        if not kernel_basename.startswith("oro-kernel-"):
            error(
                "current progspace file is not an Oro kernel; restart GDB and try again"
            )
            return

        kernel_arch = kernel_path.rsplit("-", 1)[1]
        limine_basename = f"oro-limine-{kernel_arch}"
        limine_path = path.join(path.dirname(kernel_path), limine_basename)

        if kernel_arch == "x86_64":
            efi_basename = None
        elif kernel_arch == "aarch64":
            efi_basename = "BOOTAA64.EFI"
        else:
            error(f"unsupported architecture: {kernel_arch}")
            return

        # Does it exist?
        if not path.exists(limine_path):
            error(f"limine bootloader file not found: {limine_path}")
            return

        # Assemble an ISO directory for Limine
        iso_dir = path.join(get_site_packages_dir(), "limine_iso")
        if path.exists(iso_dir):
            log("removing existing Limine ISO directory:", iso_dir)
            shutil.rmtree(iso_dir)

        os.mkdir(iso_dir)
        os.mkdir(path.join(iso_dir, "EFI"))
        os.mkdir(path.join(iso_dir, "EFI", "BOOT"))

        def copyfile(src, dst):
            log("copy:", src, "->", dst)
            shutil.copyfile(src, dst)

        copy_limine = lambda filename: copyfile(
            path.join(limine_dir, filename), path.join(iso_dir, filename)
        )

        copy_limine("limine-uefi-cd.bin")
        copy_limine("limine-bios-cd.bin")
        copy_limine("limine-bios.sys")

        with open(path.join(iso_dir, "limine.cfg"), "w") as f:
            f.write(
                """
                TIMEOUT=0
                GRAPHICS=no
                VERBOSE=yes
                RANDOMISE_MEMORY=no
                INTERFACE_BRANDING=Oro Operating System
                INTERFACE_BRANDING_COLOR=5
                SERIAL=yes

                :Oro Operating System
                PROTOCOL=limine
                KERNEL_PATH=boot:///oro-limine
                SERIAL=yes
            """
            )

        copyfile(kernel_path, path.join(iso_dir, "oro-kernel"))
        copyfile(limine_path, path.join(iso_dir, "oro-limine"))

        if efi_basename:
            copyfile(
                path.join(limine_dir, efi_basename),
                path.join(iso_dir, "EFI", "BOOT", efi_basename),
            )

        # Run xorriso to create the ISO
        log("creating Limine ISO")
        iso_path = path.join(get_site_packages_dir(), f"oro-{kernel_arch}.iso")
        subprocess.run(
            [
                "xorriso",
                "-as",
                "mkisofs",
                "-b",
                "limine-bios-cd.bin",
                "-no-emul-boot",
                "-boot-load-size",
                "4",
                "-boot-info-table",
                "--efi-boot",
                "limine-uefi-cd.bin",
                "-efi-boot-part",
                "--efi-boot-image",
                "--protective-msdos-label",
                iso_dir,
                "-o",
                iso_path,
            ],
            check=True,
        )

        log("running Limine post-ISO step")
        subprocess.run(
            [path.join(limine_dir, "limine"), "bios-install", iso_path],
            check=True,
        )

        # Do we have QEMU for the specified arch?
        if kernel_arch == "x86_64":
            qemu_arg0 = "qemu-system-x86_64"
            qemu_args = [
                "-cdrom",
                iso_path,
                "-serial",
                "stdio",
                "-no-reboot",
                "-no-shutdown",
                "-smp",
                f"cores={num_cores}",
                "-m",
                "1G",
                "-S",
                *rest_args,
            ]
        elif kernel_arch == "aarch64":
            # either QEMU_EFI env var, defaulting to "/usr/share/qemu-efi-aarch64/QEMU_EFI.fd"
            qemu_efi_path = os.getenv(
                "QEMU_EFI", "/usr/share/qemu-efi-aarch64/QEMU_EFI.fd"
            )
            if not os.path.exists(qemu_efi_path):
                error(f"QEMU_EFI path does not exist: {qemu_efi_path}")
                error(f"set QEMU_EFI to the correct path and try again")
                error(
                    f"alternatively, install the QEMU UEFI firmware package for your distro (e.g. qemu-efi-aarch64)"
                )
                return

            qemu_arg0 = "qemu-system-aarch64"
            qemu_args = [
                "-M",
                "virt",
                "-cpu",
                "cortex-a57",
                "-no-reboot",
                "-no-shutdown",
                "-serial",
                "stdio",
                "-cdrom",
                iso_path,
                "-smp",
                f"{num_cores}",
                "-m",
                "1G",
                "-bios",
                qemu_efi_path,
                *rest_args,
            ]
        else:
            error(f"unsupported QEMU architecture: {kernel_arch}")
            return

        qemu_program = shutil.which(qemu_arg0)
        if not qemu_program:
            error(
                f"{qemu_arg0} is required to boot the kernel, but was not found on PATH"
            )
            return

        if not check_bin_dep(qemu_program, "--version"):
            error(
                f"'{qemu_program} --version' failed to execute; ensure QEMU is installed correctly and try again"
            )
            return

        # Spawn the process in the background and get a handle to it
        QEMU.spawn_and_connect([qemu_program, *qemu_args])

        # Switch to the Limine bootloader if requested
        if switch:
            log("switching to Limine executable")
            with gdb_util.parameter("confirm", False):
                gdb.execute(f"file {limine_path}", to_string=False, from_tty=True)

        if auto_continue:
            log("setting _start breakpoint")
            gdb.Breakpoint("_start", internal=True, temporary=True, qualified=True)

            log("kernel booted; continuing execution")
            gdb.execute("continue", to_string=False, from_tty=True)
        else:
            log("kernel booted; use \x1b[1mcontinue\x1b[22m to start execution")
            log("(note: _start breakpoint was NOT set)")


BootCmd()
BootCmdLimine()
