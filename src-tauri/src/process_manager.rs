#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID, MAX_PATH};
#[cfg(windows)]
use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, SetPriorityClass, SetProcessAffinityMask,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    IDLE_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
    PROCESS_VM_READ, PROCESS_QUERY_LIMITED_INFORMATION,
};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
#[cfg(windows)]
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
#[cfg(windows)]
use windows::core::PCWSTR;

const TARGET_PROCESSES: [&str; 2] = ["SGuard64.exe", "SGuardSvc64.exe"];

#[derive(Clone, serde::Serialize)]
pub struct ProcessStatus {
    pub name: String,
    pub pid: u32,
    pub adjusted: bool,
    pub error: Option<String>,
}

/// 启用 SeDebugPrivilege 权限（需要管理员）
#[cfg(windows)]
pub fn enable_debug_privilege() -> Result<(), String> {
    unsafe {
        let mut token_handle: HANDLE = HANDLE::default();
        let process = GetCurrentProcess();
        
        OpenProcessToken(process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token_handle)
            .map_err(|e| format!("无法打开进程令牌: {:?}", e))?;

        let mut luid = LUID::default();
        let priv_name: Vec<u16> = "SeDebugPrivilege\0".encode_utf16().collect();
        
        LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(priv_name.as_ptr()), &mut luid)
            .map_err(|e| format!("无法查找权限: {:?}", e))?;

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        AdjustTokenPrivileges(token_handle, false, Some(&mut tp), 0, None, None)
            .map_err(|e| format!("无法调整权限: {:?}", e))?;

        let _ = CloseHandle(token_handle);
        log::info!("已启用 SeDebugPrivilege");
        Ok(())
    }
}

/// 获取系统 CPU 核心数
#[cfg(windows)]
fn get_cpu_count() -> u32 {
    unsafe {
        let mut sys_info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut sys_info);
        sys_info.dwNumberOfProcessors
    }
}

/// 获取最后一个 CPU 核心的亲和性掩码
#[cfg(windows)]
fn get_last_core_mask() -> usize {
    let cpu_count = get_cpu_count();
    1usize << (cpu_count - 1)
}

/// 调整进程优先级和 CPU 亲和性
#[cfg(windows)]
fn adjust_process(pid: u32) -> Result<(), String> {
    unsafe {
        // 尝试多种权限组合
        let handle: HANDLE = OpenProcess(
            PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        ).or_else(|_| {
            OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        }).map_err(|e| format!("无法打开进程 {}: {:?}", pid, e))?;

        // 设置为最低优先级
        if let Err(e) = SetPriorityClass(handle, IDLE_PRIORITY_CLASS) {
            let _ = CloseHandle(handle);
            return Err(format!("设置优先级失败: {:?}", e));
        }

        // 设置 CPU 亲和性到最后一个核心
        let mask = get_last_core_mask();
        if let Err(e) = SetProcessAffinityMask(handle, mask) {
            let _ = CloseHandle(handle);
            return Err(format!("设置 CPU 亲和性失败: {:?}", e));
        }

        let _ = CloseHandle(handle);
        Ok(())
    }
}

/// 获取进程名称（使用多种方法尝试）
#[cfg(windows)]
fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        // 先尝试 PROCESS_QUERY_LIMITED_INFORMATION（对受保护进程更友好）
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid))
            .ok()?;

        // 方法1: 使用 QueryFullProcessImageNameW（推荐，对 PPL 进程有效）
        let mut path_buf = [0u16; MAX_PATH as usize];
        let mut size = path_buf.len() as u32;
        
        if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(path_buf.as_mut_ptr()), &mut size).is_ok() && size > 0 {
            let path = String::from_utf16_lossy(&path_buf[..size as usize]);
            let _ = CloseHandle(handle);
            // 从完整路径提取文件名
            if let Some(name) = path.rsplit('\\').next() {
                return Some(name.to_string());
            }
            return Some(path);
        }

        // 方法2: 回退到 GetModuleBaseNameW
        let mut name_buf = [0u16; MAX_PATH as usize];
        let len = GetModuleBaseNameW(handle, None, &mut name_buf);
        let _ = CloseHandle(handle);

        if len > 0 {
            Some(String::from_utf16_lossy(&name_buf[..len as usize]))
        } else {
            None
        }
    }
}

/// 仅扫描目标进程（不调整）
#[cfg(windows)]
pub fn scan_only() -> Vec<ProcessStatus> {
    let _ = enable_debug_privilege();
    
    let mut results = Vec::new();
    
    unsafe {
        let mut pids = [0u32; 4096];
        let mut bytes_returned = 0u32;
        
        if EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        ).is_ok() {
            let count = bytes_returned as usize / std::mem::size_of::<u32>();
            
            for &pid in &pids[..count] {
                if pid == 0 { continue; }
                
                if let Some(name) = get_process_name(pid) {
                    if TARGET_PROCESSES.iter().any(|&t| t.eq_ignore_ascii_case(&name)) {
                        results.push(ProcessStatus {
                            name,
                            pid,
                            adjusted: false,
                            error: None,
                        });
                    }
                }
            }
        }
    }
    
    results
}

#[cfg(not(windows))]
pub fn scan_only() -> Vec<ProcessStatus> {
    vec![]
}

/// 限制指定 PID 的进程
#[cfg(windows)]
pub fn limit_process(pid: u32) -> ProcessStatus {
    let _ = enable_debug_privilege();
    
    let name = get_process_name(pid).unwrap_or_else(|| format!("PID:{}", pid));
    
    let (adjusted, error) = match adjust_process(pid) {
        Ok(_) => {
            log::info!("已限制进程: {} PID={}", name, pid);
            (true, None)
        }
        Err(e) => {
            log::error!("限制进程失败: {} PID={}, 错误: {}", name, pid, e);
            (false, Some(e))
        }
    };
    
    ProcessStatus { name, pid, adjusted, error }
}

#[cfg(not(windows))]
pub fn limit_process(pid: u32) -> ProcessStatus {
    ProcessStatus {
        name: format!("PID:{}", pid),
        pid,
        adjusted: false,
        error: Some("非 Windows 系统".to_string()),
    }
}
