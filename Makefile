ifndef ORO_QEMU_BIN
ORO_QEMU_BIN="/usr/local/bin"
endif

CARGO_FLAGS:=
ifdef ORO_TEST_MMIO
CARGO_FLAGS+=--features orok-test/mmio
endif

ifdef PROFILE
CARGO_FLAGS+=--profile $(PROFILE)
endif

ifdef RELEASE
CARGO_FLAGS+=--release
endif

CLIPPY_ARGS:=
ifdef FIX
CLIPPY_ARGS+=--fix --allow-dirty
endif

ifdef CLIPPY_JSON
CLIPPY_ARGS+=--message-format=json \
	--error-format=json \
	--json=diagnostic-rendered-ansi
endif

SYSNAME := $(shell uname -s)
QEMU_SUFFIX:=
ifeq ($(SYSNAME),Darwin)
QEMU_SUFFIX:=-unsigned
endif

CARGO_UNSTABLE:=\
	-Zunstable-options \
	-Zbuild-std=core,compiler_builtins,alloc \
	-Zbuild-std-features=compiler-builtins-mem

REQUESTED_ISO_ARCHES := $(filter iso-x86_64 iso-aarch64 iso-riscv64,$(MAKECMDGOALS))
ifeq ($(REQUESTED_ISO_ARCHES),)
    ISO_ARCH_TARGETS := iso-x86_64 iso-aarch64 iso-riscv64
else
    ISO_ARCH_TARGETS := $(REQUESTED_ISO_ARCHES)
endif

.PHONY: all
all: build

.PHONY: ci
ci: test build lint

.PHONY: build
build: x86_64 aarch64 riscv64

.PHONY: clippy
clippy: clippy-x86_64 clippy-aarch64 clippy-riscv64 clippy-test

.PHONY: lint lint-fmt lint-clippy lint-docs
lint: lint-fmt lint-clippy lint-docs
lint-fmt:
	cargo fmt --all --check
lint-clippy: clippy
lint-docs:
	env RUSTFLAGS="-D warnings" RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all

.PHONY: x86_64 aarch64 riscv64
x86_64: x86_64-limine x86_64-kernel
aarch64: aarch64-limine aarch64-kernel
riscv64: riscv64-limine riscv64-kernel

.PHONY: x86_64-limine
x86_64-limine:
	@cargo build \
		--target=./orok-arch-x86_64/x86_64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: aarch64-limine
aarch64-limine:
	@cargo build \
		--target=./orok-arch-aarch64/aarch64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: riscv64-limine
riscv64-limine:
	@cargo build \
		--target=./orok-arch-riscv64/riscv64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: x86_64-kernel
x86_64-kernel:
	@cargo build \
		--target=./orok-arch-x86_64/x86_64-unknown-oro.json \
		-p orok-kernel \
		--bin oro-kernel \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: aarch64-kernel
aarch64-kernel:
	@cargo build \
		--target=./orok-arch-aarch64/aarch64-unknown-oro.json \
		-p orok-kernel \
		--bin oro-kernel \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: riscv64-kernel
riscv64-kernel:
	@cargo build \
		--target=./orok-arch-riscv64/riscv64-unknown-oro.json \
		-p orok-kernel \
		--bin oro-kernel \
		$(CARGO_FLAGS) \
		$(CARGO_UNSTABLE)

.PHONY: iso
iso: iso-clean $(ISO_ARCH_TARGETS) iso-build

.PHONY: iso-build
iso-build: .limine/limine | iso-x86_64 iso-aarch64 iso-riscv64
	mkdir -p target/iso/boot/limine target/iso/EFI/BOOT
	cp \
		.limine/limine-uefi-cd.bin \
		.limine/limine-bios-cd.bin \
		.limine/limine-bios.sys \
		.limine/limine.conf \
		target/iso/boot/limine
	cp \
		.limine/BOOTX64.EFI \
		.limine/BOOTAA64.EFI \
		.limine/BOOTRISCV64.EFI \
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

.PHONY: iso-x86_64
iso-x86_64: iso-clean x86_64
    # Note the use of '-' instead of '_' below; \
	# Limine's configuration 'arch' variable uses a hyphen!
	cp \
		target/x86_64-unknown-oro/debug/oro-limine \
		target/iso/oro-limine-x86-64
	cp \
		target/x86_64-unknown-oro/debug/oro-kernel \
		target/iso/oro-kernel-x86-64

.PHONY: iso-aarch64
iso-aarch64: iso-clean aarch64
	cp \
		target/aarch64-unknown-oro/debug/oro-limine \
		target/iso/oro-limine-aarch64
	cp \
		target/aarch64-unknown-oro/debug/oro-kernel \
		target/iso/oro-kernel-aarch64

.PHONY: iso-riscv64
iso-riscv64: iso-clean riscv64
	cp \
		target/riscv64-unknown-oro/debug/oro-limine \
		target/iso/oro-limine-riscv64
	cp \
		target/riscv64-unknown-oro/debug/oro-kernel \
		target/iso/oro-kernel-riscv64

.PHONY: iso-clean
iso-clean:
	rm -rf target/iso
	mkdir -p target/iso

.limine/limine:
	make -C .limine limine

.PHONY: run-x86_64
run-x86_64: iso-x86_64 iso-build
	qemu-system-x86_64$(QEMU_SUFFIX) \
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
run-aarch64: iso-aarch64 iso-build
	@echo '[ORO] if the following command fails due to missing QEMU_EFI.fd,'
	@echo '[ORO] run `apt install qemu-efi-aarch64`.'
	qemu-system-aarch64$(QEMU_SUFFIX) \
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

.PHONY: run-riscv64
run-riscv64: iso-riscv64 iso-build target/RISCV_VIRT_VARS.fd
	@echo '[ORO] if the following command fails due to missing RISCV_VIRT_CODE.fd,'
	@echo '[ORO] run `apt install qemu-efi-riscv64`.'
	qemu-system-riscv64$(QEMU_SUFFIX) \
		-M virt \
		-cpu max \
		-no-reboot \
		-no-shutdown \
		-serial stdio \
		-cdrom target/oro.iso \
		-m 512 \
		-smp cores=4 \
		-drive if=pflash,format=raw,unit=0,file=/usr/share/qemu-efi-riscv64/RISCV_VIRT_CODE.fd,readonly=on \
		-drive if=pflash,format=raw,unit=1,file=target/RISCV_VIRT_VARS.fd,readonly=off \
		-monitor telnet:localhost:4444,nowait,server \
		$(QEMU_ARGS)

target/RISCV_VIRT_VARS.fd:
	cp /usr/share/qemu-efi-riscv64/RISCV_VIRT_VARS.fd target/RISCV_VIRT_VARS.fd

.PHONY: clippy-x86_64
clippy-x86_64:
	@cargo clippy \
		--target=./orok-arch-x86_64/x86_64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CLIPPY_ARGS) \
		$(CARGO_UNSTABLE)

.PHONY: clippy-aarch64
clippy-aarch64:
	@cargo clippy \
		--target=./orok-arch-aarch64/aarch64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CLIPPY_ARGS) \
		$(CARGO_UNSTABLE)

.PHONY: clippy-riscv64
clippy-riscv64:
	@cargo clippy \
		--target=./orok-arch-riscv64/riscv64-unknown-oro.json \
		-p orok-boot-limine \
		--bin oro-limine \
		$(CARGO_FLAGS) \
		$(CLIPPY_ARGS) \
		$(CARGO_UNSTABLE)

.PHONY: clippy-test
clippy-test:
	@cargo clippy \
		-p orok-test \
		-p orok-test-tui \
		-p orok-test-harness \
		$(CARGO_FLAGS) \
		$(CLIPPY_ARGS)

.PHONY: test
test:
	cargo test --all

.PHONY: tui
tui:
	cargo run --release -p orok-test-tui
