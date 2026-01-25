ifndef ORO_QEMU_BIN
ORO_QEMU_BIN="/usr/local/bin"
endif

FEATURES:=
ifdef ORO_TEST_MMIO
FEATURES:=--features orok-test/mmio
endif

CLIPPY_ARGS:=
ifdef FIX
CLIPPY_ARGS:=--fix --allow-dirty
endif

.PHONY: all
all: build

.PHONY: build
build: x86_64 aarch64

.PHONY: clippy
clippy: clippy-x86_64 clippy-aarch64

.PHONY: x86_64 aarch64
x86_64: x86_64-limine
aarch64: aarch64-limine

.PHONY: x86_64-limine
x86_64-limine:
	cargo build \
		--target=./orok-arch-x86_64/x86_64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine-x86_64 \
		$(FEATURES) \
		-Zunstable-options \
			-Zbuild-std=core,compiler_builtins,alloc \
			-Zbuild-std-features=compiler-builtins-mem

.PHONY: aarch64-limine
aarch64-limine:
	cargo build \
		--target=./orok-arch-aarch64/aarch64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine-aarch64 \
		$(FEATURES) \
		-Zunstable-options \
			-Zbuild-std=core,compiler_builtins,alloc \
			-Zbuild-std-features=compiler-builtins-mem

.PHONY: iso
iso: x86_64 aarch64 .limine/limine
	rm -rf target/iso
	mkdir -p target/iso/boot/limine target/iso/EFI/BOOT
	cp \
		.limine/limine-uefi-cd.bin \
		.limine/limine-bios-cd.bin \
		.limine/limine-bios.sys \
		.limine/limine.conf \
		target/iso/boot/limine
    # Note the change from '_' to '-' below \
	# Limine's configuration 'arch' variable uses a hyphen
	cp \
		target/x86_64-unknown-oro/debug/oro-limine-x86_64 \
		target/iso/oro-limine-x86-64
	cp \
		target/aarch64-unknown-oro/debug/oro-limine-aarch64 \
		target/iso/oro-limine-aarch64
	cp \
		.limine/BOOTX64.EFI \
		.limine/BOOTAA64.EFI \
		target/iso/EFI/BOOT
	xorriso \
        -as mkisofs \
        -R -r -J \
        -b boot/limine/limine-bios-cd.bin \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -hfsplus \
        -apm-block-size 2048 \
        --efi-boot boot/limine/limine-uefi-cd.bin \
        -efi-boot-part \
        --efi-boot-image \
        --protective-msdos-label \
		"target/iso" -o "target/oro.iso"
	.limine/limine bios-install target/oro.iso

.limine/limine:
	make -C .limine limine

.PHONY: run-x86_64
run-x86_64: iso
	/src/oro-os/oro-qemu/build/qemu-system-x86_64 \
		-M q35 \
		-cdrom target/oro.iso \
		-serial stdio \
		-no-reboot \
		-no-shutdown \
		-smp cores=4 \
		-monitor telnet:localhost:4444,nowait,server \
		-d guest_errors \
		$(QEMU_ARGS)

.PHONY: run-aarch64
run-aarch64: iso
	@echo '[ORO] if the following command fails due to missing QEMU_EFI.fd,'
	@echo '[ORO] run `apt install qemu-efi-aarch64`.'
	qemu-system-aarch64 \
		-M virt \
		-cpu cortex-a57 \
		-no-reboot \
		-no-shutdown \
		-serial stdio \
		-cdrom target/oro.iso \
		-m 512 \
		-smp cores=4 \
		-bios /usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
		-monitor telnet:localhost:4444,nowait,server \
		$(QEMU_ARGS)

.PHONY: clippy-x86_64
clippy-x86_64:
	cargo clippy \
		--target=./orok-arch-x86_64/x86_64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine-x86_64 \
		$(FEATURES) \
		$(CLIPPY_ARGS) \
		-Zunstable-options \
			-Zbuild-std=core,compiler_builtins,alloc \
			-Zbuild-std-features=compiler-builtins-mem

.PHONY: clippy-aarch64
clippy-aarch64:
	cargo clippy \
		--target=./orok-arch-aarch64/aarch64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine-aarch64 \
		$(FEATURES) \
		$(CLIPPY_ARGS) \
		-Zunstable-options \
			-Zbuild-std=core,compiler_builtins,alloc \
			-Zbuild-std-features=compiler-builtins-mem