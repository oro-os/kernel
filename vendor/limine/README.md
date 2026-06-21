# The Limine Boot Protocol For Rust
This is the new version of the `limine` crate.

An example project using this crate can be found [here](https://github.com/robotman2412/limine-boot-demo).

*Note: The API has singificantly changed between 0.5.x and 0.6.x*

# Supported Features
Base revision: 6

Requests:
 - Bootloader Info
 - Executable Command Line
 - Firmware Type
 - Stack Size
 - HHDM (Higher Half Direct Map)
 - Framebuffer
 - Paging Mode
 - MP (Multiprocessor)
 - RISC-V BSP Hart ID
 - Memory Map
 - Entry Point
 - Executable File
 - Module
 - RSDP
 - SMBIOS
 - EFI System Table
 - EFI Memory Map
 - Date at Boot
 - Executable Address
 - Device Tree Blob
 - Bootloader Performance
 - Timestamp counter frequency
 - Flanterm FB init params

# License
This crate is available as either [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE)
