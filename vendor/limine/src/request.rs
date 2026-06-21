// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::{
    cell::UnsafeCell,
    ffi::{CStr, c_char},
    ops::Deref,
    ptr::null_mut,
};

use crate::{
    COMMON_MAGIC,
    entrypoint::EntryPoint,
    file::File,
    flanterm::FlantermParams,
    framebuffer::{Framebuffer, FramebufferRev1},
    memmap,
    module::InternalModule,
    paging::PagingMode,
};

/// An abstract request to the bootloader.
#[repr(C)]
pub struct Request<Resp, Req = ()> {
    magic: [u64; 2],
    id: [u64; 2],
    revision: u64,
    response: UnsafeCell<*mut Response<Resp>>,
    request: Req,
}
unsafe impl<Resp, Req> Send for Request<Resp, Req> {}
unsafe impl<Resp, Req> Sync for Request<Resp, Req> {}

impl<Resp, Req> Request<Resp, Req> {
    /// Base construction for all requests.
    /// The rationale for unsafe here is that mismatching the ID and request type is undefined behaviour, even though this is a private function.
    pub const unsafe fn new_raw(id: [u64; 2], revision: u64, request: Req) -> Self {
        Self {
            magic: COMMON_MAGIC,
            id,
            revision,
            response: UnsafeCell::new(null_mut()),
            request,
        }
    }

    /// Some requests have multiple revisions.
    /// For such requests, this number identifies what version the executable needs.
    /// The bootloader may respond with a lower revision if it doesn't support the requested revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Get the response to this request.
    pub fn response(&self) -> Option<&'static Response<Resp>> {
        unsafe { core::mem::transmute(self.response.get().read_volatile()) }
    }
}

impl<Resp, Req> Deref for Request<Resp, Req> {
    type Target = Req;

    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

/// An abstract response from the bootloader.
#[repr(C)]
pub struct Response<T> {
    revision: u64,
    data: T,
}
unsafe impl<Resp> Send for Response<Resp> {}
unsafe impl<Resp> Sync for Response<Resp> {}

impl<T> Response<T> {
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

impl<T> Deref for Response<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/* ==== REQUEST AND RESPONSE DEFINITIONS ==== */

/// Get bootloader information.
pub type BootloaderInfoRequest = Request<BootloaderInfoRespData>;

/// Name and version of the bootloader.
/// Response to [`BootloaderInfoRequest`].
pub type BootloaderInfoResponse = Response<BootloaderInfoRespData>;

#[repr(C)]
pub struct BootloaderInfoRespData {
    name: *const c_char,
    version: *const c_char,
}

impl BootloaderInfoRespData {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.name).to_str().unwrap() }
    }

    pub fn version(&self) -> &str {
        unsafe { CStr::from_ptr(self.version).to_str().unwrap() }
    }
}

impl BootloaderInfoRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0xf55038d8e2a1202f, 0x279426fcf5f59740], 0, ()) }
    }
}

/// Get the executable's command line argument string.
pub type ExecutableCmdlineRequest = Request<ExecutableCmdlineRespData>;

/// The executable's command-line argument string.
/// Response to [`ExecutableCmdlineRequest`].
pub type ExecutableCmdlineResponse = Response<ExecutableCmdlineRespData>;

#[repr(C)]
pub struct ExecutableCmdlineRespData {
    cmdline: *const c_char,
}

impl ExecutableCmdlineRespData {
    pub fn cmdline(&self) -> &str {
        unsafe { CStr::from_ptr(self.cmdline).to_str().unwrap() }
    }
}

impl ExecutableCmdlineRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0x4b161536e598651e, 0xb390ad4a2f1f303a], 0, ()) }
    }
}

/// Get the firmware type.
pub type FirmwareTypeRequest = Request<FirmwareTypeRespData>;

/// The firmware type.
/// Response to [`FirmwareTypeRequest`].
pub type FirmwareTypeResponse = Response<FirmwareTypeRespData>;

#[repr(C)]
pub struct FirmwareTypeRespData {
    pub firmware_type: u64,
}

impl FirmwareTypeRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0x8c2f75d90bef28a8, 0x7045a4688eac00c3], 0, ()) }
    }
}

/// Request a specific stack size.
/// Presence of the response indicates compliance to the request.
pub type StackSizeRequest = Request<StackSizeRespData, u64>;

pub struct StackSizeRespData;

impl StackSizeRequest {
    pub const fn new(size: u64) -> Self {
        unsafe { Request::new_raw([0x224ef0460a8e8926, 0xe1cb0fc25f46ea3d], 0, size) }
    }
}

/// Get information about the Higher-Half Direct Map.
pub type HhdmRequest = Request<HhdmRespData>;

/// Information about the Higher-Half Direct Map.
/// Reponse to [`HhdmRequest`].
pub type HhdmRepsonse = Response<HhdmRespData>;

#[repr(C)]
pub struct HhdmRespData {
    pub offset: u64,
}

impl HhdmRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0x48dcf1cb8ad2b852, 0x63984e959a98244b], 0, ()) }
    }
}

/// Request a framebuffer be created.
pub type FramebufferRequest = Request<FramebufferRespData>;

/// Available framebuffer(s).
/// Response to [`FramebufferRequest`].
pub type FramebufferResponse = Response<FramebufferRespData>;

#[repr(C)]
pub struct FramebufferRespData {
    framebuffer_count: u64,
    framebuffers: *const (),
}

impl Response<FramebufferRespData> {
    /// Revision 0 framebuffers (with one fixed mode).
    /// You can also try [`Self::framebuffers_rev1`] to get framebuffers with multiple modes.
    pub fn framebuffers(&self) -> &[&Framebuffer] {
        unsafe {
            &*core::ptr::slice_from_raw_parts(
                self.framebuffers as _,
                self.framebuffer_count as usize,
            )
        }
    }

    /// Revision 1 framebuffers (possibly with one or more alternate modes).
    /// If this returns [`None`], use the result from [`Self::framebuffers`] instead.
    pub fn framebuffers_rev1(&self) -> Option<&[&FramebufferRev1]> {
        if self.revision < 1 {
            return None;
        }
        Some(unsafe {
            &*core::ptr::slice_from_raw_parts(
                self.framebuffers as _,
                self.framebuffer_count as usize,
            )
        })
    }
}

impl FramebufferRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0x9d5827dcd881dd75, 0xa3148604f6fab11b], 0, ()) }
    }
}

#[repr(C)]
pub struct FlantermParamsRespData {
    entry_count: u64,
    entries: *const *const FlantermParams,
}

/// Flanterm FB init params request.
pub type FlantermParamsRequest = Request<FlantermParamsRespData>;

/// Flanterm FB init params response.
pub type FlantermParamsResponse = Response<FlantermParamsRespData>;

impl FlantermParamsRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x3259399fe7c5f126, 0xe01c1c8c5db9d1a9], 0, ()) }
    }
}

impl FlantermParamsRespData {
    pub fn entries(&self) -> &[&FlantermParams] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.entries as _, self.entry_count as usize) }
    }
}

/// Set paging mode request.
pub type PagingModeRequest = Request<PagingModeRespData, PagingModeReqData>;

/// Paging mode response.
/// Reponse to [`PagingModeRequest`].
pub type PagingModeResponse = Response<PagingModeRespData>;

#[repr(C)]
pub struct PagingModeReqData {
    /// Preferred mode.
    pub mode: PagingMode,
    /// Maximum mode; the bootloader will not load the OS if unsatisfied.
    pub max_mode: PagingMode,
    /// Minimum mode; the bootloader will not load the OS if unsatisfied.
    pub min_mode: PagingMode,
}

#[repr(C)]
pub struct PagingModeRespData {
    /// Currently active paging mode.
    pub mode: PagingMode,
}

impl PagingModeRequest {
    /// Request that accepts any mode but prefers the maximum available paging mode.
    pub const PREFER_MAXIMUM: PagingModeRequest =
        Self::new(PagingMode::MAX, PagingMode::MAX, PagingMode::MIN);

    pub const fn new(mode: PagingMode, max_mode: PagingMode, min_mode: PagingMode) -> Self {
        if !((min_mode as u64) <= (mode as u64) && (mode as u64) <= (max_mode as u64)) {
            // Paging mode request preconditions violated: min_mode <= mode <= max_mode.
            loop {}
        }
        unsafe {
            Self::new_raw(
                [0x95c1a0edab0944cb, 0xa4e5cb3842f7488a],
                0,
                PagingModeReqData {
                    mode,
                    max_mode,
                    min_mode,
                },
            )
        }
    }

    /// Request that accepts one exact paging mode.
    pub const fn new_exact(mode: PagingMode) -> Self {
        Self::new(mode, mode, mode)
    }
}

/// Intialize Multi-Processing request.
pub type MpRequest = Request<MpRespData, u64>;

/// Multi-Processing response.
/// Reponse to [`MpRequest`].
pub type MpResponse = Response<MpRespData>;

// Unlike other requests, the response format is arch-specific, so it's not defined here.
pub use crate::mp::MpRespData;

impl MpRequest {
    pub const fn new(flags: u64) -> Self {
        unsafe { Self::new_raw([0x95a67b819a1b857e, 0xa0b61b723b6a73e0], 0, flags) }
    }
}

/// Get memory-map request.
pub type MemmapRequest = Request<MemmapRespData>;

/// Memory-map response.
/// Reponse to [`MemmapRequest`].
pub type MemmapResponse = Response<MemmapRespData>;

#[repr(C)]
pub struct MemmapRespData {
    entry_count: u64,
    entries: *const (),
}

impl MemmapRespData {
    pub fn entries(&self) -> &[&memmap::Entry] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.entries as _, self.entry_count as usize) }
    }
}

impl MemmapRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x67cf3d9d378a806f, 0xe304acdfc50c3c62], 0, ()) }
    }
}

/// Alternate kernel entrypoint request.
/// Overrides the executable file's entrypoint address.
/// Presence of the response indicates compliance to the request.
pub type EntryPointRequest = Request<EntrypointRespData, EntryPoint>;

pub struct EntrypointRespData;

impl EntryPointRequest {
    pub const fn new(entry: EntryPoint) -> Self {
        unsafe { Self::new_raw([0x13d86c035a1cd3e1, 0x2b0caa89d8f3026a], 0, entry) }
    }
}

/// Get executable file information request.
pub type ExecutableFileRequest = Request<ExecutableFileRespData>;

/// Executable file information response.
/// Reponse to [`ExecutableFileRequest`].
pub type ExecutableFileResponse = Response<ExecutableFileRespData>;

#[repr(C)]
pub struct ExecutableFileRespData {
    executable_file: *const File,
}

impl ExecutableFileRespData {
    pub fn executable_file(&self) -> &File {
        unsafe { &*self.executable_file }
    }
}

impl ExecutableFileRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0xad97e90e83f1ed67, 0x31eb5d1c5ff23b69], 0, ()) }
    }
}

/// Get modules request.
pub type ModulesRequest = Request<ModulesRespData, ModulesReqData>;

/// Loaded modules response.
/// Response to [`ModulesRequest`].
pub type ModulesResponse = Response<ModulesRespData>;

#[repr(C)]
pub struct ModulesReqData {
    internal_module_count: u64,
    internal_modules: *const *const InternalModule,
}

#[repr(C)]
pub struct ModulesRespData {
    module_count: u64,
    modules: *const (),
}

impl ModulesRespData {
    pub fn modules(&self) -> &[&File] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.modules as _, self.module_count as usize) }
    }
}

impl ModulesRequest {
    /// Revision 0 loaded modules (without ability to request internal modules).
    /// If internal modules are desired, use [`Self::new_rev1`].
    pub const fn new() -> Self {
        unsafe {
            Self::new_raw(
                [0x3e7e279702be32af, 0xca1c4f3bd1280cee],
                0,
                ModulesReqData {
                    internal_module_count: 0,
                    internal_modules: core::ptr::null(),
                },
            )
        }
    }

    /// Revision 1 loaded modules (with ability to request internal modules).
    pub const fn new_rev1(internal_modules: &'static [&'static InternalModule]) -> Self {
        unsafe {
            Self::new_raw(
                [0x3e7e279702be32af, 0xca1c4f3bd1280cee],
                1,
                ModulesReqData {
                    internal_module_count: internal_modules.len() as u64,
                    internal_modules: internal_modules.as_ptr() as _,
                },
            )
        }
    }
}

/// Get RSDP address request.
/// WARNING: For base revision 3 only, this is a physical address, whereas other revisions use a virtual address.
pub type RsdpRequest = Request<RsdpRespData>;

/// RSDP address response.
/// WARNING: For base revision 3 only, this is a physical address, whereas other revisions use a virtual address.
/// Response to [`RsdpRequest`].
pub type RsdpResponse = Response<RsdpRespData>;

#[repr(C)]
pub struct RsdpRespData {
    pub address: *mut (),
}

impl RsdpRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0xc5e77b6b397e7b43, 0x27637845accdcf3c], 0, ()) }
    }
}

/// Get SMBIOS entrypoint request.
/// If SMBIOS is not available, no response will be provided.
/// WARNING: For base revisions 3 and 4 only, this is a physical address, whereas other revisions use a virtual address.
pub type SmbiosRequest = Request<SmbiosRespData>;

/// SMBIOS entrypoint response.
/// WARNING: For base revisions 3 and 4 only, this is a physical address, whereas other revisions use a virtual address.
/// Response to [`SmbiosRequest`].
pub type SmbiosResponse = Response<SmbiosRespData>;

#[repr(C)]
pub struct SmbiosRespData {
    pub entry_32: *const (),
    pub entry_64: *const (),
}

impl SmbiosRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x9e9046f11e095391, 0xaa4a520fefbde5ee], 0, ()) }
    }
}

/// Get EFI system table address request.
/// If EFI is not available, no response will be provided.
/// WARNING: For base revisions 3 and 4 only, this is a physical address, whereas other revisions use a virtual address.
pub type EfiRequest = Request<EfiRespData>;

/// EFI system table address response.
/// WARNING: For base revisions 3 and 4 only, this is a physical address, whereas other revisions use a virtual address.
/// Response to [`EfiRequest`].
pub type EfiResponse = Response<EfiRespData>;

#[repr(C)]
pub struct EfiRespData {
    pub address: *const (),
}

impl EfiRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x5ceba5163eaaf6d6, 0x0a6981610cf65fcc], 0, ()) }
    }
}

/// Get EFI memory map request.
/// WARNING: For base revisions 3 and 4 only, this is a physical address, whereas other revisions use a virtual address.
pub type EfiMemmapRequest = Request<EfiMemmapRespData>;

/// EFI memory map response.
/// Response to [`EfiMemmapRequest`].
pub type EfiMemmapResponse = Response<EfiMemmapRespData>;

#[repr(C)]
pub struct EfiMemmapRespData {
    memmap: *const (),
    memmap_size: u64,
    pub desc_size: u64,
    pub desc_version: u64,
}

impl EfiMemmapRespData {
    pub fn memmap(&self) -> &[u8] {
        unsafe {
            &*core::ptr::slice_from_raw_parts(self.memmap as *const u8, self.memmap_size as usize)
        }
    }
}

impl EfiMemmapRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x7df62a431d6872d5, 0xa4fcdfb3e57306c8], 0, ()) }
    }
}

/// Get date at boot request.
pub type DateAtBootRequest = Request<DateAtBootRespData>;

/// Date at boot response.
/// Response to [`DateAtBootRequest`].
pub type DateAtBootResponse = Response<DateAtBootRespData>;

#[repr(C)]
pub struct DateAtBootRespData {
    pub timestamp: i64,
}

impl DateAtBootRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x502746e184c088aa, 0xfbc5ec83e6327893], 0, ()) }
    }
}

/// Get executable address request.
pub type ExecutableAddressRequest = Request<ExecutableAddressRespData>;

/// Executable address response.
/// Response to [`ExecutableAddressRequest`].
pub type ExecutableAddressResponse = Response<ExecutableAddressRespData>;

#[repr(C)]
pub struct ExecutableAddressRespData {
    pub physical_base: u64,
    pub virtual_base: u64,
}

impl ExecutableAddressRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x71ba76863cc55f63, 0xb2644a48c516a487], 0, ()) }
    }
}

/// Get DTB request.
pub type DtbRequest = Request<DtbRespData>;

/// DTB response.
/// Response to [`DtbRequest`].
pub type DtbResponse = Response<DtbRespData>;

#[repr(C)]
pub struct DtbRespData {
    pub dtb_ptr: *const (),
}

impl DtbRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0xb40ddb48fb54bac7, 0x545081493f81ffb7], 0, ()) }
    }
}

/// Get BSP hartid request.
pub type BspHartidRequest = Request<BspHartidRespData>;

/// BSP hartid response.
/// Response to [`BspHartidRequest`].
pub type BspHartidResponse = Response<BspHartidRespData>;

#[repr(C)]
pub struct BspHartidRespData {
    pub bsp_hartid: u64,
}

impl BspHartidRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x1369359f025525f9, 0x2ff2a56178391bb6], 0, ()) }
    }
}

/// Get bootloader performance request.
pub type BootloaderPerformanceRequest = Request<BootloaderPerformanceRespData>;

/// Bootloader performance response.
/// Response to [`BootloaderPerformanceRequest`].
pub type BootloaderPerformanceResponse = Response<BootloaderPerformanceRespData>;

#[repr(C)]
pub struct BootloaderPerformanceRespData {
    pub reset_usec: u64,
    pub init_usec: u64,
    pub exec_usec: u64,
}

impl BootloaderPerformanceRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x6b50ad9bf36d13ad, 0xdc4c7e88fc759e17], 0, ()) }
    }
}

/// Keep x86_64 I/O MMU enabled request.
/// Currently has no effect on other platforms.
pub type KeepIommuRequest = Request<KeepIommuRespData>;

pub struct KeepIommuRespData;

impl KeepIommuRequest {
    pub const fn new() -> Self {
        unsafe { Self::new_raw([0x8ebaabe51f490179, 0x2aa86a59ffb4ab0f], 0, ()) }
    }
}

#[repr(C)]
pub struct TscFrequencyRespData {
    /// Frequency in Hz of the main timestamp counter.
    /// On x86, this means the TSC, for RISC-V, rdtime, and for ARM, CNTPCT_EL0.
    pub frequency: u64,
}

/// Get timestamp counter frequency request.
pub type TscFrequencyRequest = Request<TscFrequencyRespData>;

/// Get timestamp counter frequency response.
pub type TscFrequencyResponse = Response<TscFrequencyRespData>;

impl TscFrequencyRequest {
    pub const fn new() -> Self {
        unsafe { Request::new_raw([0x10f2ee1d87d195e4, 0xf747a2b78f6ddb31], 0, ()) }
    }
}
