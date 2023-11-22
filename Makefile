.PHONY: all clean fmt lint clippy x64 x64-limine x64-limine.iso x64-limine.qemu x64-limine.pxe

ORO_VERSION = $(shell cargo metadata --format-version=1 | jq -r '.packages | map(select(.name == "oro-kernel")) | .[].version')

ifeq ($(DEBUG),1)
	ifeq ($(TEST),1)
		override RELEASE = test-dev
		CARGO_FLAGS += --profile=test-dev
	else
		override RELEASE = debug
		CARGO_FLAGS += --profile=dev
	endif
else
	ifeq ($(TEST),1)
		override RELEASE = test-release
		CARGO_FLAGS += --profile=test-release
	else
		override RELEASE = release
		CARGO_FLAGS += --release
	endif
endif

all: x64 x64-limine.iso x64-limine.pxe

clean:
	rm -rf target

fmt:
	cargo fmt --all

lint:
	cargo fmt -- --check --verbose

clippy:
	env cargo clippy $(CARGO_FLAGS) --target=./triple/x64.json -Zunstable-options -Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem --all -- -D clippy::all

# oro x64
x64: target/x64/$(RELEASE)/oro-kernel
.PHONY: target/x64/$(RELEASE)/oro-kernel
target/x64/$(RELEASE)/oro-kernel:
	env RUSTFLAGS="-Z macro-backtrace" cargo build -p oro-kernel --target=./triple/x64.json -Zunstable-options -Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem $(CARGO_FLAGS)

# oro x64-limine
x64-limine: x64 target/x64/$(RELEASE)/oro-boot-limine-x64
.PHONY: target/x64/$(RELEASE)/oro-boot-limine-x64
target/x64/$(RELEASE)/oro-boot-limine-x64:
	env RUSTFLAGS="-Z macro-backtrace" cargo build -p oro-boot-limine-x64 --target=./triple/x64.json -Zunstable-options -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem $(CARGO_FLAGS)
target/x64/$(RELEASE)/.limine/limine: oro-boot-limine-x64/bootloader/limine
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
oro-boot-limine-x64/bootloader/limine: oro-boot-limine-x64/bootloader/limine.c
	$(MAKE) -C oro-boot-limine-x64/bootloader limine
oro-boot-limine-x64/bootloader/limine.c:
	git submodule update --init --recursive --depth=1 oro-boot-limine-x64/bootloader

# oro-x64-limine (run QEMU w/ ISO)
x64-limine.qemu: x64-limine.iso
	qemu-system-x86_64 -cdrom target/out/oro-$(ORO_VERSION)-x64-limine-$(RELEASE).iso -serial stdio $(QEMUFLAGS) -no-reboot -no-shutdown

# oro x64-limine (ISO)
x64-limine.iso: x64 target/out/oro-$(ORO_VERSION)-x64-limine-$(RELEASE).iso
target/out/oro-$(ORO_VERSION)-x64-limine-$(RELEASE).iso: $(addprefix target/x64/$(RELEASE)/.limine/,iso/oro-kernel iso/oro-boot-limine-x64 iso/limine-uefi-cd.bin iso/limine-bios-cd.bin iso/limine-bios.sys iso/limine.cfg limine)
	@mkdir -p "$(dir $@)"
	xorriso -as mkisofs -b limine-bios-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table --efi-boot limine-uefi-cd.bin -efi-boot-part --efi-boot-image --protective-msdos-label "target/x64/$(RELEASE)/.limine/iso" -o "$@"
	target/x64/$(RELEASE)/.limine/limine bios-install "$@"
target/x64/$(RELEASE)/.limine/iso/limine.cfg: oro-boot-limine-x64/limine.cfg
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/.limine/iso/%: oro-boot-limine-x64/bootloader/%
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/.limine/iso/oro-kernel: target/x64/$(RELEASE)/oro-kernel
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/.limine/iso/oro-boot-limine-x64: target/x64/$(RELEASE)/oro-boot-limine-x64
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"

# oro x64-limine (PXE BIOS/UEFI bootable)
x64-limine.pxe: x64 x64-limine $(addprefix target/x64/$(RELEASE)/pxe/,oro-boot-limine-x64 oro-kernel limine.cfg BOOTX64.EFI limine-bios.sys limine-bios-pxe.bin)
target/x64/$(RELEASE)/pxe/limine.cfg: oro-boot-limine-x64/limine.cfg
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/pxe/oro-boot-limine-x64: target/x64/$(RELEASE)/oro-boot-limine-x64
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/pxe/oro-kernel: target/x64/$(RELEASE)/oro-kernel
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
target/x64/$(RELEASE)/pxe/%: oro-boot-limine-x64/bootloader/%
	@mkdir -p "$(dir $@)"
	cp "$<" "$@"
