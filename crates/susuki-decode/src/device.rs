//! HW backend selection (design 4.2/4.3).
//! Two-stage: device existence vs initialization failure, per REQ-DC-002.
//! Unix probes NVDEC and VAAPI; Windows probes D3D11VA only (it covers every
//! GPU vendor there, so NVDEC is not offered on Windows).

#[cfg(unix)]
use std::path::PathBuf;

use crate::error::DecodeError;
use crate::sys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Nvdec,
    Vaapi,
    D3d11va,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Existence {
    /// no device/driver at all: `auto` may select CPU (§4.3 step 1-2)
    Absent,
    /// driver/node present but probe or enumeration failed: NOT absence (§4.3)
    InitFailed(String),
    Present,
}

#[cfg(unix)]
#[derive(Debug)]
pub struct Probe {
    pub nvdec: Existence,
    pub vaapi_nodes: Vec<PathBuf>,
}

#[cfg(unix)]
impl Probe {
    /// §4.3 step 1: distinct probing of absence vs init failure.
    // 既定値ではなくシステムプローブ結果で構築するため Default は作らない
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Probe {
            nvdec: probe_nvdec(),
            vaapi_nodes: probe_vaapi_nodes(),
        }
    }

    fn vaapi_absent(&self) -> bool {
        self.vaapi_nodes.is_empty()
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub struct Probe {
    pub d3d11va: Existence,
    /// DXGI adapter indices of hardware adapters, in enumeration order
    pub d3d11va_adapters: Vec<u32>,
}

#[cfg(windows)]
impl Probe {
    /// §4.3 step 1: distinct probing of absence vs init failure.
    // 既定値ではなくシステムプローブ結果で構築するため Default は作らない
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        match probe_dxgi_adapters() {
            Ok(adapters) if adapters.is_empty() => Probe {
                d3d11va: Existence::Absent,
                d3d11va_adapters: adapters,
            },
            Ok(adapters) => Probe {
                d3d11va: Existence::Present,
                d3d11va_adapters: adapters,
            },
            Err(msg) => Probe {
                d3d11va: Existence::InitFailed(msg),
                d3d11va_adapters: Vec::new(),
            },
        }
    }
}

/// NVDEC existence: try dlopen of libcuda.so.1 then cuInit + cuDeviceGetCount.
/// dlopen failure = driver not installed (Absent).
/// cuInit/enumeration failure with the library present = InitFailed (§4.3 step 1,
/// 2026-10-02 user decision). device count 0 = Absent.
#[cfg(unix)]
fn probe_nvdec() -> Existence {
    let lib = match unsafe { libloading::Library::new("libcuda.so.1") } {
        Ok(l) => l,
        Err(_) => return Existence::Absent,
    };
    unsafe {
        let cu_init: libloading::Symbol<extern "C" fn(i32) -> i32> = match lib.get(b"cuInit\0") {
            Ok(s) => s,
            Err(_) => return Existence::InitFailed("cuInit symbol not found".to_string()),
        };
        let rc = cu_init(0);
        if rc != 0 {
            return Existence::InitFailed(format!("cuInit failed rc={rc}"));
        }
        let cu_count: libloading::Symbol<extern "C" fn(*mut i32) -> i32> =
            match lib.get(b"cuDeviceGetCount\0") {
                Ok(s) => s,
                Err(_) => {
                    return Existence::InitFailed("cuDeviceGetCount symbol not found".to_string())
                }
            };
        let mut n: i32 = 0;
        let rc = cu_count(&mut n as *mut i32);
        if rc != 0 {
            return Existence::InitFailed(format!("cuDeviceGetCount failed rc={rc}"));
        }
        if n == 0 {
            return Existence::Absent;
        }
    }
    Existence::Present
}

/// VAAPI existence (§4.3 step 1): count of /dev/dri/renderD* nodes.
#[cfg(unix)]
fn probe_vaapi_nodes() -> Vec<PathBuf> {
    let mut nodes = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/dev/dri") {
        for entry in rd.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("renderD") {
                nodes.push(entry.path());
            }
        }
    }
    nodes.sort();
    nodes
}

/// D3D11VA existence (§4.3 step 1): DXGI hardware adapters. Software adapters
/// (Microsoft Basic Render Driver / WARP) cannot decode and count as absent,
/// as does a system without DXGI support at all (DXGI_ERROR_UNSUPPORTED, the
/// counterpart of libcuda failing to load). Any other failure to create the
/// factory or to enumerate is an init failure.
/// The indices match FFmpeg's d3d11va device string (its adapter number).
#[cfg(windows)]
fn probe_dxgi_adapters() -> Result<Vec<u32>, String> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
        DXGI_ERROR_UNSUPPORTED,
    };
    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(e) if e.code() == DXGI_ERROR_UNSUPPORTED => return Ok(Vec::new()),
        Err(e) => return Err(format!("CreateDXGIFactory1 failed: {e}")),
    };
    let mut adapters = Vec::new();
    for i in 0u32.. {
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(format!("EnumAdapters1({i}) failed: {e}")),
        };
        let desc = unsafe { adapter.GetDesc1() }
            .map_err(|e| format!("GetDesc1 for adapter {i} failed: {e}"))?;
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 {
            adapters.push(i);
        }
    }
    Ok(adapters)
}

/// An initialized hardware device context (owning an AVBufferRef).
pub struct HwDevice {
    pub kind: BackendKind,
    pub buffer: *mut sys::AVBufferRef,
    pub device_desc: String,
}

impl Drop for HwDevice {
    fn drop(&mut self) {
        if !self.buffer.is_null() {
            unsafe { sys::av_buffer_unref(&mut self.buffer) };
        }
    }
}

fn create_hwdevice(kind: BackendKind, device: Option<&str>) -> Result<HwDevice, DecodeError> {
    let (hw_type, desc) = match kind {
        BackendKind::Nvdec => (sys::AVHWDeviceType_AV_HWDEVICE_TYPE_CUDA, "cuda"),
        BackendKind::Vaapi => (sys::AVHWDeviceType_AV_HWDEVICE_TYPE_VAAPI, "vaapi"),
        BackendKind::D3d11va => (sys::AVHWDeviceType_AV_HWDEVICE_TYPE_D3D11VA, "d3d11va"),
        BackendKind::Cpu => unreachable!("cpu has no hwdevice"),
    };
    let cdev = match device {
        Some(d) => std::ffi::CString::new(d)
            .map_err(|_| DecodeError::Runtime("device spec contains NUL".to_string()))?,
        None => std::ffi::CString::default(),
    };
    let mut buffer: *mut sys::AVBufferRef = std::ptr::null_mut();
    let rc = unsafe {
        sys::av_hwdevice_ctx_create(
            &mut buffer,
            hw_type,
            if device.is_some() {
                cdev.as_ptr()
            } else {
                std::ptr::null()
            },
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return Err(DecodeError::Runtime(format!(
            "{desc}: av_hwdevice_ctx_create failed: {}",
            sys::err_str(rc)
        )));
    }
    Ok(HwDevice {
        kind,
        buffer,
        device_desc: desc.to_string(),
    })
}

/// §4.3 full selection for `--backend auto`:
/// returns (backend, optional hwdevice). CPU chosen only when ALL candidates are
/// Absent and none InitFailed. Any init failure without a success -> Runtime (5).
#[cfg(unix)]
pub fn select_auto(
    probe: &Probe,
    device: Option<&str>,
) -> Result<(BackendKind, Option<HwDevice>), DecodeError> {
    let mut any_init_failure: Option<String> = None;

    // NVDEC first by priority (§4.3 step 3)
    let nvdec_ok = match &probe.nvdec {
        Existence::Absent => false,
        Existence::InitFailed(msg) => {
            // cuInit/enumeration failure is an initialization failure (§4.3 step 1)
            any_init_failure = Some(format!("nvdec: {msg}"));
            false
        }
        Existence::Present => true,
    };
    if nvdec_ok {
        match create_hwdevice(BackendKind::Nvdec, device) {
            Ok(hw) => return Ok((BackendKind::Nvdec, Some(hw))),
            Err(e) => any_init_failure = Some(e.message().to_string()),
        }
    }

    // VAAPI: existing render nodes in order; --device overrides the node (§4.3 step 3)
    if !probe.vaapi_absent() {
        let nodes: Vec<Option<String>> = match device {
            Some(d) => vec![Some(d.to_string())],
            None => probe
                .vaapi_nodes
                .iter()
                .map(|n| Some(n.to_string_lossy().into_owned()))
                .collect(),
        };
        for node in nodes {
            match create_hwdevice(BackendKind::Vaapi, node.as_deref()) {
                Ok(hw) => return Ok((BackendKind::Vaapi, Some(hw))),
                Err(e) => any_init_failure = Some(e.message().to_string()),
            }
        }
    }

    let all_absent = matches!(probe.nvdec, Existence::Absent) && probe.vaapi_absent();
    finish_auto(all_absent, any_init_failure)
}

/// §4.3 `--backend auto` on Windows: D3D11VA over the hardware adapters in
/// order (--device overrides the adapter), else CPU only when absent.
#[cfg(windows)]
pub fn select_auto(
    probe: &Probe,
    device: Option<&str>,
) -> Result<(BackendKind, Option<HwDevice>), DecodeError> {
    let mut any_init_failure: Option<String> = None;
    match &probe.d3d11va {
        Existence::Absent => {}
        Existence::InitFailed(msg) => any_init_failure = Some(format!("d3d11va: {msg}")),
        Existence::Present => {
            let adapters: Vec<String> = match device {
                Some(d) => vec![d.to_string()],
                None => probe
                    .d3d11va_adapters
                    .iter()
                    .map(|i| i.to_string())
                    .collect(),
            };
            for adapter in adapters {
                match create_hwdevice(BackendKind::D3d11va, Some(&adapter)) {
                    Ok(hw) => return Ok((BackendKind::D3d11va, Some(hw))),
                    Err(e) => any_init_failure = Some(e.message().to_string()),
                }
            }
        }
    }
    let all_absent = matches!(probe.d3d11va, Existence::Absent);
    finish_auto(all_absent, any_init_failure)
}

/// No GPU device present at all, and no init failure -> CPU (§4.3 step 2).
fn finish_auto(
    all_absent: bool,
    any_init_failure: Option<String>,
) -> Result<(BackendKind, Option<HwDevice>), DecodeError> {
    if all_absent && any_init_failure.is_none() {
        return Ok((BackendKind::Cpu, None));
    }

    Err(DecodeError::Runtime(format!(
        "no GPU backend could be initialized ({}); CPU fallback is forbidden for supported inputs (REQ-DC-002)",
        any_init_failure.unwrap_or_else(|| "unknown".to_string())
    )))
}

/// Explicit backend request (§4.3 step 4): absence or init failure -> 5.
pub fn select_explicit(
    kind: BackendKind,
    device: Option<&str>,
) -> Result<Option<HwDevice>, DecodeError> {
    match kind {
        BackendKind::Cpu => Ok(None),
        k => create_hwdevice(k, device).map(Some),
    }
}
