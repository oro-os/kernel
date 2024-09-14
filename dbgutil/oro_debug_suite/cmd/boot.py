import gdb  # type: ignore
import os
from os import path
from ..log import log, error, warn, debug
from .. import gdb_util
import subprocess
from ..service import QEMU, SYMBOLS
from ..service.autosym import SYM_KERNEL_TRANSFER


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
        oro boot limine [-sbCK] [-n <num_cores>]

    Options:
        -S, --no-switch        Don't switch to the Limine executable before booting.
                               Specifying this will break many of the trackers.
                               Probably not a good idea to use it.
        -n, --num_cores        Specify the number of CPU cores to emulate (default: 1).
        -C, --no-continue      Do not automatically continue execution after booting.
        -K, --no-autokernel    Do not automatically load the kernel image during transfer.
                               (Only useful with --switch)
        -b, --break            Break at the start of the bootloader or kernel image after transfer
                               (whatever comes first).
        -M, --no-env-modules   Don't load a module list from the `ORO_ROOT_MODULES` environment variable.
        -m, --module <path>    Include a module from `<path>` to be loaded onto the root ring.
                               Can be specified multiple times. In addition to this option, a
                               semi-colon separated list of modules can be specified in the
                               `ORO_ROOT_MODULES` environment variable.
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

        modules = []
        load_modules_from_env = True
        switch = True
        autoload_kernel = True
        num_cores = 1
        auto_continue = True
        break_at_start = False

        argi = 0
        while argi < len(args):
            arg = args[argi]

            if arg in ["--no-switch", "-S"]:
                switch = False
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
            elif arg in ["--no-autokernel", "-K"]:
                autoload_kernel = False
            elif arg in ["--break", "-b"]:
                break_at_start = True
            elif arg in ["--no-env-modules", "-M"]:
                load_modules_from_env = False
            elif arg in ["--module", "-m"]:
                if argi + 1 >= len(args):
                    error("missing argument for --module")
                    return

                modules.append(args[argi + 1])
                argi += 1
            elif arg == "--":
                rest_args = args[argi + 1 :]
                break
            else:
                error(f"unknown argument: {arg}")
                return

            argi += 1

        if load_modules_from_env:
            env_modules = os.getenv("ORO_ROOT_MODULES")
            if env_modules:
                modules.extend(env_modules.split(";"))

        # Make sure they exist
        for module in modules:
            if not path.exists(module):
                error(f"module file not found: {module}")
                return

        # Fetch Limine if it doesn't exist
        limine_dir = fetch_limine()

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
            qemu_arg0 = "qemu-system-x86_64"
        elif kernel_arch == "aarch64":
            qemu_arg0 = "qemu-system-aarch64"
            efi_basename = "BOOTAA64.EFI"
        else:
            error(f"unsupported architecture: {kernel_arch}")
            return

        # Does it exist?
        if not path.exists(limine_path):
            error(f"limine bootloader file not found: {limine_path}")
            return

        # Do we have everything we need to make the ISO?
        if not check_bin_deps("xorriso", "git", "make"):
            error(
                "missing required PATH utilities to build Limine ISO; install them and try again"
            )
            return

        iso_path = path.join(get_site_packages_dir(), f"oro-{kernel_arch}.iso")
        extra_iso_files = []

        # Do we have QEMU?
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

        # Set up the architecture-specific QEMU arguments
        if kernel_arch == "x86_64":
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

            qemu_args = [
                "-M",
                "virt",
                "-cpu",
                "cortex-a57",
                "-no-reboot",
                "-no-shutdown",
                "-serial",
                "stdio",
                "-smp",
                f"{num_cores}",
                "-m",
                "1G",
                "-bios",
                qemu_efi_path,
                *rest_args,
            ]

            # Generate the device tree blob
            dtb_path = path.join(get_site_packages_dir(), "oro-aarch64-qemu.dtb")
            dtb_gen_args = [qemu_program, *qemu_args, "-machine", f"dumpdtb={dtb_path}"]
            debug("generating QEMU device tree blob with args:", repr(dtb_gen_args))
            subprocess.run(
                dtb_gen_args,
                check=True,
            )
            extra_iso_files.append((dtb_path, "oro-device-tree.dtb"))

            qemu_args.append("-cdrom")
            qemu_args.append(iso_path)
        else:
            error(f"unsupported QEMU architecture: {kernel_arch}")
            return

        # Create an ISO directory for Limine
        iso_dir = path.join(get_site_packages_dir(), "limine_iso")
        if path.exists(iso_dir):
            log("removing existing Limine ISO directory:", iso_dir)
            shutil.rmtree(iso_dir)

        os.mkdir(iso_dir)
        os.mkdir(path.join(iso_dir, "EFI"))
        os.mkdir(path.join(iso_dir, "EFI", "BOOT"))

        # Populate the ISO directory
        def copyfile(src, dst):
            log("copy:", src, "->", dst)
            shutil.copyfile(src, dst)

        copy_limine = lambda filename: copyfile(
            path.join(limine_dir, filename), path.join(iso_dir, filename)
        )

        copy_limine("limine-uefi-cd.bin")
        copy_limine("limine-bios-cd.bin")
        copy_limine("limine-bios.sys")

        module_config = "\n".join(
            [f"MODULE_PATH=boot:///{path.basename(m)}" for m in modules]
        )

        with open(path.join(iso_dir, "limine.cfg"), "w") as f:
            f.write(
                f"""
                TIMEOUT=0
                GRAPHICS=no
                VERBOSE=yes
                RANDOMISE_MEMORY=no
                INTERFACE_BRANDING=Oro Operating System
                INTERFACE_BRANDING_COLOR=5
                SERIAL=yes
                KASLR=no

                :Oro Operating System
                PROTOCOL=limine
                KERNEL_PATH=boot:///oro-limine
                SERIAL=yes
                {module_config}
                """
            )

        copyfile(kernel_path, path.join(iso_dir, "oro-kernel"))
        copyfile(limine_path, path.join(iso_dir, "oro-limine"))

        if efi_basename:
            copyfile(
                path.join(limine_dir, efi_basename),
                path.join(iso_dir, "EFI", "BOOT", efi_basename),
            )

        for src, dst in extra_iso_files:
            copyfile(src, path.join(iso_dir, dst))
        for module in modules:
            copyfile(module, path.join(iso_dir, path.basename(module)))

        # Run xorriso to create the ISO
        log("creating Limine ISO")
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

        # Spawn the process in the background and get a handle to it
        QEMU.spawn_and_connect([qemu_program, *qemu_args])

        # Switch to the Limine bootloader if requested
        if switch:
            log("switching to Limine executable")
            with gdb_util.parameter("confirm", False):
                gdb.execute(f"file {limine_path}", to_string=False, from_tty=True)

            if autoload_kernel:
                # Set an auto-switch breakpoint if we found one
                kernel_will_switch_sym = SYMBOLS.get_if_tracked(SYM_KERNEL_TRANSFER)
                if kernel_will_switch_sym:
                    log("setting kernel switch breakpoint")
                    SwitchKernelBreakpoint(kernel_will_switch_sym, kernel_path)
                else:
                    warn(
                        "no kernel switch symbol found; will not automatically switch to kernel image"
                    )

        if auto_continue:
            if break_at_start:
                log("setting _start breakpoint")
                gdb.Breakpoint("_start", internal=True, temporary=True, qualified=True)

            log("kernel booted; continuing execution")
            gdb.execute("continue", to_string=False, from_tty=True)
        else:
            log("kernel booted; use \x1b[1mcontinue\x1b[22m to start execution")
            log("(note: _start breakpoint was NOT set)")


class SwitchKernelBreakpoint(gdb.Breakpoint):
    def __init__(self, at, switch_to_file):
        super(SwitchKernelBreakpoint, self).__init__(
            at, internal=True, temporary=True, qualified=True
        )
        self.silent = True
        self._switch_to_file = switch_to_file

    def stop(self):
        debug(
            "preboot environment is about to jump to kernel; switching to kernel image file"
        )
        with gdb_util.parameter("confirm", False):
            gdb.execute(f"file {self._switch_to_file}", to_string=False, from_tty=True)
        return False  # don't stop


BootCmd()
BootCmdLimine()
