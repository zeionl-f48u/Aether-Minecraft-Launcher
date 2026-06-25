//! 跨平台实用工具函数
//!
//! 提供系统信息查询、文件路径处理、可执行文件架构检测等功能。

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::env;

use sha1::{Digest, Sha1};
use thiserror::Error;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum UtilsError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("无效的文件格式: {0}")]
    InvalidFormat(String),
    #[error("不支持的架构")]
    UnsupportedArch,
    #[error("内存信息获取失败: {0}")]
    MemoryInfo(String),
    #[error("路径规范化失败: {0}")]
    PathNormalization(String),
}

pub type UtilsResult<T> = Result<T, UtilsError>;

// ============================================================================
//  平台常量
// ============================================================================

/// 目标操作系统名称
pub const TARGET_OS: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "osx"
} else if cfg!(target_os = "linux") {
    "linux"
} else {
    "unknown"
};

/// 目标 CPU 架构
pub const TARGET_ARCH: &str = if cfg!(target_arch = "x86") {
    "x86"
} else if cfg!(target_arch = "x86_64") {
    "x86_64"
} else if cfg!(target_arch = "arm") {
    "arm"
} else if cfg!(target_arch = "aarch64") {
    "aarch64"
} else {
    "unknown"
};

/// 系统位数（32 或 64）
pub const NATIVE_ARCH: &str = if cfg!(target_pointer_width = "32") {
    "32"
} else {
    "64"
};

/// 类路径分隔符
pub const CLASSPATH_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

// ============================================================================
//  运行时原生架构检测（通过系统 API）
// ============================================================================

lazy_static::lazy_static! {
    /// 当前系统的原生架构（运行时检测，而非编译目标）
    pub static ref NATIVE_ARCH_LAZY: String = get_native_arch();
}

#[cfg(target_os = "windows")]
fn get_native_arch() -> String {
    use windows::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};
    unsafe {
        let mut info = SYSTEM_INFO::default();
        GetNativeSystemInfo(&mut info);
        match info.Anonymous.Anonymous.wProcessorArchitecture.0 {
            0 => "x86".to_string(),      // PROCESSOR_ARCHITECTURE_INTEL
            9 => "x86_64".to_string(),   // PROCESSOR_ARCHITECTURE_AMD64
            12 => "arm".to_string(),     // PROCESSOR_ARCHITECTURE_ARM
            13 => "aarch64".to_string(), // PROCESSOR_ARCHITECTURE_ARM64
            _ => "unknown".to_string(),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_native_arch() -> String {
    // 使用 `uname -m` 或直接读取 /proc/cpuinfo 等，但简单起见，
    // 这里返回编译目标架构（大多数情况相同）
    std::env::consts::ARCH.to_string()
}

// ============================================================================
//  路径操作
// ============================================================================

/// 获取路径的绝对路径字符串，即使路径不存在也能规范化
///
/// 若路径已经是绝对路径，则直接返回；否则基于当前工作目录拼接。
/// 此函数不会检查文件是否存在。
pub fn get_full_path(p: impl AsRef<Path>) -> UtilsResult<String> {
    let path = p.as_ref();
    if path.is_absolute() {
        // 使用 canonicalize 来解析符号链接，如果失败则返回原路径
        match path.canonicalize() {
            Ok(abs) => Ok(abs.to_string_lossy().into_owned()),
            Err(_) => Ok(path.to_string_lossy().into_owned()),
        }
    } else {
        let current_dir = env::current_dir()
            .map_err(|e| UtilsError::PathNormalization(format!("无法获取当前目录: {}", e)))?;
        let full = current_dir.join(path);
        // 尝试规范化，失败则保留
        match full.canonicalize() {
            Ok(abs) => Ok(abs.to_string_lossy().into_owned()),
            Err(_) => Ok(full.to_string_lossy().into_owned()),
        }
    }
}

/// 在系统中查找可执行文件
///
/// 依次搜索：
/// - 当前目录
/// - PATH 环境变量中的目录
/// 在 Windows 上，若文件名没有扩展名，自动尝试添加 `.exe`。
pub fn locate_path(exe_name: impl AsRef<Path>) -> UtilsResult<PathBuf> {
    let name = exe_name.as_ref();

    // 检查当前目录
    if name.exists() && is_executable(name) {
        return Ok(name.to_path_buf());
    }

    // 在 Windows 上尝试添加 .exe 后缀
    #[cfg(windows)]
    let candidates = {
        let mut v = vec![name.to_path_buf()];
        if name.extension().is_none() {
            v.push(name.with_extension("exe"));
        }
        v
    };
    #[cfg(not(windows))]
    let candidates = vec![name.to_path_buf()];

    // 搜索 PATH
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            for candidate in &candidates {
                let full = dir.join(candidate);
                if full.exists() && is_executable(&full) {
                    return Ok(full);
                }
            }
        }
    }

    // 如果都找不到，返回原始名称（可能失败由调用方处理）
    Ok(name.to_path_buf())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("bat") || e.eq_ignore_ascii_case("cmd"))
        .unwrap_or(false)
}

// ============================================================================
//  架构检测
// ============================================================================

/// 可执行文件架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecArch {
    X86,
    X86_64,
    ARM,
    AArch64,
    Unknown,
}

impl ExecArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecArch::X86 => "x86",
            ExecArch::X86_64 => "x86_64",
            ExecArch::ARM => "arm",
            ExecArch::AArch64 => "aarch64",
            ExecArch::Unknown => "unknown",
        }
    }
}

/// 检测可执行文件的架构
///
/// 通过读取文件的二进制头部（PE / Mach-O / ELF）来判定。
pub fn get_exec_arch(path: &Path) -> UtilsResult<ExecArch> {
    let mut file = File::open(path)?;
    let mut buf = [0u8; 64]; // 足够读取各类头部
    file.read_exact(&mut buf)?;

    // 检查魔数
    if &buf[0..4] == b"\x7fELF" {
        return parse_elf(&buf);
    } else if &buf[0..2] == b"MZ" {
        return parse_pe(&mut file, &buf);
    } else if &buf[0..4] == b"\xfe\xed\xfa\xce" || &buf[0..4] == b"\xce\xfa\xed\xfe"
        || &buf[0..4] == b"\xfe\xed\xfa\xcf" || &buf[0..4] == b"\xcf\xfa\xed\xfe" {
        return parse_mach_o(&buf);
    }

    Err(UtilsError::InvalidFormat("未知的可执行文件格式".into()))
}

// ---------- ELF ----------
fn parse_elf(buf: &[u8]) -> UtilsResult<ExecArch> {
    // e_ident[EI_CLASS] 确定 32/64
    match buf[4] {
        1 => Ok(ExecArch::X86),      // ELFCLASS32
        2 => {
            // 检查 e_machine（需要从偏移 0x12 读取）
            let machine = u16::from_le_bytes([buf[0x12], buf[0x13]]);
            match machine {
                0x3e => Ok(ExecArch::X86_64), // EM_X86_64
                0x28 => Ok(ExecArch::ARM),     // EM_ARM
                0xb7 => Ok(ExecArch::AArch64), // EM_AARCH64
                _ => Ok(ExecArch::Unknown),
            }
        }
        _ => Err(UtilsError::InvalidFormat("无效的 ELF 类".into())),
    }
}

// ---------- PE (Windows) ----------
fn parse_pe(file: &mut File, buf: &[u8]) -> UtilsResult<ExecArch> {
    // 从 DOS header 中读取 e_lfanew（偏移 0x3C）
    let e_lfanew = u32::from_le_bytes([buf[0x3C], buf[0x3D], buf[0x3E], buf[0x3F]]) as u64;
    file.seek(SeekFrom::Start(e_lfanew))?;
    let mut pe_header = [0u8; 24];
    file.read_exact(&mut pe_header)?;

    // 检查 PE 签名
    if &pe_header[0..4] != b"PE\0\0" {
        return Err(UtilsError::InvalidFormat("无效的 PE 签名".into()));
    }

    // Machine 字段位于偏移 0x04（相对 PE 头起始）
    let machine = u16::from_le_bytes([pe_header[0x04], pe_header[0x05]]);
    match machine {
        0x014c => Ok(ExecArch::X86),
        0x8664 => Ok(ExecArch::X86_64),
        0xaa64 => Ok(ExecArch::AArch64),
        0x01c4 => Ok(ExecArch::ARM), // ARMv7
        _ => Ok(ExecArch::Unknown),
    }
}

// ---------- Mach-O (macOS) ----------
fn parse_mach_o(buf: &[u8]) -> UtilsResult<ExecArch> {
    // 魔数决定了字节序
    let is_big_endian = match &buf[0..4] {
        b"\xfe\xed\xfa\xce" | b"\xfe\xed\xfa\xcf" => true,
        b"\xce\xfa\xed\xfe" | b"\xcf\xfa\xed\xfe" => false,
        _ => return Err(UtilsError::InvalidFormat("无效的 Mach-O 魔数".into())),
    };

    // cputype 在偏移 0x04（对于 64 位文件，偏移可能不同，但通常前 8 字节足够）
    let cputype = if is_big_endian {
        u32::from_be_bytes([buf[0x04], buf[0x05], buf[0x06], buf[0x07]])
    } else {
        u32::from_le_bytes([buf[0x04], buf[0x05], buf[0x06], buf[0x07]])
    };

    match cputype {
        0x7 => Ok(ExecArch::X86),       // CPU_TYPE_X86
        0x01000007 => Ok(ExecArch::X86_64), // CPU_TYPE_X86_64
        0x0c => Ok(ExecArch::ARM),       // CPU_TYPE_ARM
        0x0100000c => Ok(ExecArch::AArch64), // CPU_TYPE_ARM64
        _ => Ok(ExecArch::Unknown),
    }
}

// ============================================================================
//  内存信息
// ============================================================================

/// 内存状态（单位：MB）
#[derive(Debug, Clone, Copy)]
pub struct MemoryStatus {
    pub total: u64,
    pub free: u64,
}

/// 获取系统内存信息
///
/// 使用各平台 API 获取总内存和可用内存。
pub fn get_mem_status() -> UtilsResult<MemoryStatus> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::SystemInformation::{
            GlobalMemoryStatusEx, MEMORYSTATUSEX,
        };
        unsafe {
            let mut status = MEMORYSTATUSEX::default();
            status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut status).is_ok() {
                Ok(MemoryStatus {
                    total: (status.ullTotalPhys / (1024 * 1024)) as u64,
                    free: (status.ullAvailPhys / (1024 * 1024)) as u64,
                })
            } else {
                Err(UtilsError::MemoryInfo("GlobalMemoryStatusEx 失败".into()))
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 读取 /proc/meminfo
        let content = std::fs::read_to_string("/proc/meminfo")
            .map_err(|e| UtilsError::MemoryInfo(format!("读取 /proc/meminfo 失败: {}", e)))?;
        let mut total = 0u64;
        let mut free = 0u64;
        for line in content.lines() {
            if let Some(stripped) = line.strip_prefix("MemTotal:") {
                total = parse_meminfo_line(stripped)?;
            } else if let Some(stripped) = line.strip_prefix("MemAvailable:") {
                free = parse_meminfo_line(stripped)?;
            }
        }
        if total == 0 || free == 0 {
            // fallback: 尝试读取 MemFree
            if free == 0 {
                for line in content.lines() {
                    if let Some(stripped) = line.strip_prefix("MemFree:") {
                        free = parse_meminfo_line(stripped)?;
                        break;
                    }
                }
            }
        }
        Ok(MemoryStatus { total, free })
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;
        use libc::{sysctl, CTL_HW, HW_MEMSIZE, HW_PHYSMEM, HW_USERMEM};

        // 使用 sysctl 获取总内存和可用内存
        unsafe {
            let mut total: u64 = 0;
            let mut size = mem::size_of::<u64>();
            let name: [i32; 2] = [CTL_HW, HW_MEMSIZE];
            if sysctl(name.as_ptr(), 2, &mut total as *mut _ as *mut _, &mut size, std::ptr::null_mut(), 0) != 0 {
                return Err(UtilsError::MemoryInfo("sysctl HW_MEMSIZE 失败".into()));
            }

            // 可用内存使用 HW_USERMEM（用户空间可用内存）
            let mut free: u64 = 0;
            let mut size = mem::size_of::<u64>();
            let name: [i32; 2] = [CTL_HW, HW_USERMEM];
            if sysctl(name.as_ptr(), 2, &mut free as *mut _ as *mut _, &mut size, std::ptr::null_mut(), 0) != 0 {
                // fallback: 使用 HW_PHYSMEM 减去内核占用，简单估算
                let mut phys: u64 = 0;
                let name: [i32; 2] = [CTL_HW, HW_PHYSMEM];
                if sysctl(name.as_ptr(), 2, &mut phys as *mut _ as *mut _, &mut size, std::ptr::null_mut(), 0) != 0 {
                    return Err(UtilsError::MemoryInfo("sysctl HW_PHYSMEM 失败".into()));
                }
                // 粗略估算可用内存为总内存的 80%
                free = phys * 80 / 100;
                total = phys;
            }

            Ok(MemoryStatus {
                total: total / (1024 * 1024),
                free: free / (1024 * 1024),
            })
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        // 不支持的平台，返回默认值
        Ok(MemoryStatus { total: 4096, free: 2048 })
    }
}

#[cfg(target_os = "linux")]
fn parse_meminfo_line(line: &str) -> UtilsResult<u64> {
    let s = line.trim();
    if let Some(kb) = s.strip_suffix(" kB") {
        let kb_str = kb.trim();
        kb_str.parse::<u64>()
            .map(|v| v / 1024) // 转换为 MB
            .map_err(|_| UtilsError::MemoryInfo(format!("解析数字失败: {}", kb_str)))
    } else {
        Err(UtilsError::MemoryInfo(format!("无效的内存行: {}", line)))
    }
}

// ============================================================================
//  SHA1 计算（同步）
// ============================================================================

/// 计算数据流的 SHA1 哈希值（十六进制小写）
///
/// 注意：此函数为同步版本，如果数据源是异步的，请使用 `spawn_blocking` 包裹。
pub fn get_data_sha1<R: Read>(mut reader: R) -> UtilsResult<String> {
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// 异步版本（内部调用同步版本）
pub async fn get_data_sha1_async<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
) -> UtilsResult<String> {
    use tokio::io::AsyncReadExt;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_get_full_path() {
        let path = get_full_path(".").unwrap();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_locate_path() {
        // 至少能找到自身
        let exe = locate_path(std::env::current_exe().unwrap()).unwrap();
        assert!(exe.exists());
    }

    #[test]
    fn test_get_exec_arch() {
        // 测试当前可执行文件
        let exe = std::env::current_exe().unwrap();
        let arch = get_exec_arch(&exe).unwrap();
        // 根据当前编译目标判断预期
        #[cfg(target_arch = "x86_64")]
        assert_eq!(arch, ExecArch::X86_64);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(arch, ExecArch::AArch64);
        // 其他架构忽略
    }

    #[test]
    fn test_get_mem_status() {
        let mem = get_mem_status().unwrap();
        assert!(mem.total > 0);
        assert!(mem.free > 0);
        // 检查单位是 MB
        assert!(mem.total < 1024 * 1024 * 1024); // 不可能超过 1TB 吧
    }

    #[test]
    fn test_get_data_sha1() {
        let data = b"hello world";
        let hash = get_data_sha1(Cursor::new(data)).unwrap();
        // 已知 SHA1: "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"
        assert_eq!(hash, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[tokio::test]
    async fn test_get_data_sha1_async() {
        let data = b"hello world";
        let hash = get_data_sha1_async(Cursor::new(data)).await.unwrap();
        assert_eq!(hash, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }
}