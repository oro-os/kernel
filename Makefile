.PHONY: all clean fmt lint clippy x86_64 x86_64-limine x86_64-limine.iso x86_64-limine.qemu x86_64-limine.pxe-uefi

ORO_VERSION = $(shell cargo metadata --format-version=1 | jq -r '.packages | map(select(.name == "oro-kernel")) | .[].version')

ifeq ($(DEBUG),1)
override RELEASE = debug
else
override RELEASE = release
CARGO_FLAGS += --release
endif

all: x86_64 x86_64-limine.iso x86_64-limine.pxe-uefi

clean:
	rm -rf target

fmt:
	cargo fmt --all

lint:
	cargo fmt -- --check --verbose

clippy:
	env cargo clippy $(CARGO_FLAGS) --target=./triple/x86_64.json -Zunstable-options -Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem --all -- -D clippy::all

# oro x86_64
.PHONY: target/x86_64/$(RELEASE)/oro-kernel
target/x86_64/$(RELEASE)/oro-kernel:
	env RUSTFLAGS="-Z macro-backtrace" cargo build -p oro-kernel --target=./triple/x86_64.json -Zunstable-options -Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem $(CARGO_FLAGS)

# oro x86_64-limine
x86_64-limine: x86_64 target/x86_64/$(RELEASE)/oro-boot-limine
.PHONY: target/x86_64/$(RELEASE)/oro-boot-limine
target/x86_64/$(RELEASE)/oro-boot-limine:
	env RUSTFLAGS="-Z macro-backtrace" cargo build -p oro-boot-limine --target=./triple/x86_64.json -Zunstable-options -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem $(CARGO_FLAGS)
target/x86_64/$(RELEASE)/.limine/limine: oro-boot-limine/bootloader/limine
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
oro-boot-limine/bootloader/limine: oro-boot-limine/bootloader/limine.c
	$(MAKE) -C oro-boot-limine/bootloader limine
oro-boot-limine/bootloader/limine.c:
	git submodule update --init --recursive --depth=1 oro-boot-limine/bootloader

# oro-x86_64-limine (run QEMU w/ ISO)
x86_64-limine.qemu: x86_64-limine.iso
	qemu-system-x86_64 -cdrom target/out/oro-$(ORO_VERSION)-x86_64-limine-$(RELEASE).iso -serial stdio $(QEMUFLAGS) -no-reboot -no-shutdown

# oro x86_64-limine (ISO)
x86_64-limine.iso: x86_64 target/out/oro-$(ORO_VERSION)-x86_64-limine-$(RELEASE).iso
target/out/oro-$(ORO_VERSION)-x86_64-limine-$(RELEASE).iso: $(addprefix target/x86_64/$(RELEASE)/.limine/,iso/oro-kernel iso/oro-boot-limine iso/limine-cd-efi.bin iso/limine-cd.bin iso/limine.sys iso/limine.cfg limine)
	@mkdir -p "$(dir $@)"
	xorriso -as mkisofs -b limine-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table --efi-boot limine-cd-efi.bin -efi-boot-part --efi-boot-image --protective-msdos-label "target/x86_64/$(RELEASE)/.limine/iso" -o "$@"
	target/x86_64/$(RELEASE)/.limine/limine bios-install "$@"
target/x86_64/$(RELEASE)/.limine/iso/limine.cfg: oro-boot-limine/limine.cfg
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/.limine/iso/%: oro-boot-limine/bootloader/%
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/.limine/iso/oro-kernel: target/x86_64/$(RELEASE)/oro-kernel
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/.limine/iso/oro-boot-limine: target/x86_64/$(RELEASE)/oro-boot-limine
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"

# oro x86_64-limine (PXE/UEFI bootable)
x86_64-limine.pxe-uefi: x86_64 x86_64-limine $(addprefix target/x86_64/$(RELEASE)/pxe-uefi/,oro-boot-limine oro-kernel limine.cfg BOOTX64.EFI)
target/x86_64/$(RELEASE)/pxe-uefi/limine.cfg: oro-boot-limine/limine.cfg
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/pxe-uefi/oro-boot-limine: target/x86_64/$(RELEASE)/oro-boot-limine
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/pxe-uefi/oro-kernel: target/x86_64/$(RELEASE)/oro-kernel
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x86_64/$(RELEASE)/pxe-uefi/%: oro-boot-limine/bootloader/%
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
