mod process_manager;

use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashSet;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Emitter,
};
use tauri_plugin_store::StoreExt;

static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);
// 记录已限制的 PID
static LIMITED_PIDS: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
// 最后扫描时间
static LAST_SCAN_TIME: Mutex<Option<SystemTime>> = Mutex::new(None);

fn init_limited_pids() {
    let mut pids = LIMITED_PIDS.lock().unwrap();
    if pids.is_none() {
        *pids = Some(HashSet::new());
    }
}

/// 检查 PID 是否已被限制
fn is_pid_limited(pid: u32) -> bool {
    init_limited_pids();
    let pids = LIMITED_PIDS.lock().unwrap();
    pids.as_ref().unwrap().contains(&pid)
}

/// 标记 PID 为已限制
fn mark_pid_limited(pid: u32) {
    init_limited_pids();
    let mut pids = LIMITED_PIDS.lock().unwrap();
    pids.as_mut().unwrap().insert(pid);
}

/// 仅扫描进程（保留已限制状态）
#[tauri::command]
fn scan_processes() -> Vec<process_manager::ProcessStatus> {
    init_limited_pids();
    let limited = LIMITED_PIDS.lock().unwrap();
    let limited_set = limited.as_ref().unwrap();
    
    // 更新扫描时间
    let mut last_scan = LAST_SCAN_TIME.lock().unwrap();
    *last_scan = Some(SystemTime::now());
    
    process_manager::scan_only()
        .into_iter()
        .map(|mut p| {
            if limited_set.contains(&p.pid) {
                p.adjusted = true;
            }
            p
        })
        .collect()
}

/// 获取最后扫描时间（Unix 时间戳，毫秒）
#[tauri::command]
fn get_last_scan_time() -> Option<u64> {
    let last_scan = LAST_SCAN_TIME.lock().unwrap();
    last_scan.as_ref().and_then(|time| {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as u64)
    })
}

/// 限制指定 PID 的进程（如果已限制则跳过）
#[tauri::command]
fn limit_process(pid: u32) -> process_manager::ProcessStatus {
    // 如果已经限制过，直接返回成功状态
    if is_pid_limited(pid) {
        let name = format!("PID:{}", pid);
        return process_manager::ProcessStatus {
            name,
            pid,
            adjusted: true,
            error: None,
        };
    }
    
    let result = process_manager::limit_process(pid);
    
    if result.adjusted {
        mark_pid_limited(pid);
    }
    
    result
}

/// 限制所有目标进程（跳过已限制的）
#[tauri::command]
fn limit_all() -> Vec<process_manager::ProcessStatus> {
    let scanned = process_manager::scan_only();
    
    scanned.into_iter().map(|p| {
        if is_pid_limited(p.pid) {
            process_manager::ProcessStatus {
                name: p.name,
                pid: p.pid,
                adjusted: true,
                error: None,
            }
        } else {
            let result = process_manager::limit_process(p.pid);
            if result.adjusted {
                mark_pid_limited(p.pid);
            }
            result
        }
    }).collect()
}

#[tauri::command]
fn start_monitor(app: tauri::AppHandle, interval_ms: Option<u64>) -> bool {
    if MONITOR_RUNNING.load(Ordering::Relaxed) {
        log::warn!("监控已在运行中");
        return false;
    }
    MONITOR_RUNNING.store(true, Ordering::Relaxed);
    
    let interval = Duration::from_millis(interval_ms.unwrap_or(30000));
    let app_handle = app.clone();
    
    thread::spawn(move || {
        log::info!("监控线程已启动，间隔: {:?}", interval);
        
        while MONITOR_RUNNING.load(Ordering::Relaxed) {
            // 更新扫描时间
            {
                let mut last_scan = LAST_SCAN_TIME.lock().unwrap();
                *last_scan = Some(SystemTime::now());
            }
            
            // 扫描并限制进程
            let scanned = process_manager::scan_only();
            log::info!("扫描到 {} 个目标进程", scanned.len());
            
            let mut has_changes = false;
            
            for p in scanned {
                if !is_pid_limited(p.pid) {
                    log::info!("尝试限制进程: {} (PID: {})", p.name, p.pid);
                    let result = process_manager::limit_process(p.pid);
                    if result.adjusted {
                        mark_pid_limited(p.pid);
                        log::info!("✓ 成功限制进程: {} (PID: {})", result.name, result.pid);
                        has_changes = true;
                    } else {
                        log::error!("✗ 限制进程失败: {} (PID: {}), 错误: {:?}", result.name, result.pid, result.error);
                    }
                }
            }
            
            // 如果有变化，通知前端刷新
            if has_changes {
                let _ = app_handle.emit("process-updated", ());
            }
            
            // 总是发送扫描完成事件
            let _ = app_handle.emit("scan-completed", ());
            
            thread::sleep(interval);
        }
        
        log::info!("监控线程已停止");
    });
    
    true
}

#[tauri::command]
fn stop_monitor() -> bool {
    MONITOR_RUNNING.store(false, Ordering::Relaxed);
    true
}

#[tauri::command]
fn is_monitor_running() -> bool {
    MONITOR_RUNNING.load(Ordering::Relaxed)
}

#[tauri::command]
fn get_system_info() -> SystemInfo {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
        unsafe {
            let mut sys_info: SYSTEM_INFO = std::mem::zeroed();
            GetSystemInfo(&mut sys_info);
            SystemInfo {
                cpu_count: sys_info.dwNumberOfProcessors,
                last_core_index: sys_info.dwNumberOfProcessors - 1,
            }
        }
    }
    #[cfg(not(windows))]
    {
        SystemInfo {
            cpu_count: 1,
            last_core_index: 0,
        }
    }
}

#[derive(serde::Serialize)]
struct SystemInfo {
    cpu_count: u32,
    last_core_index: u32,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            scan_processes,
            limit_process,
            limit_all,
            start_monitor,
            stop_monitor,
            is_monitor_running,
            get_system_info,
            get_last_scan_time
        ])
        .setup(|app| {
            // 检查是否静默启动
            let silent_start = {
                if let Ok(store) = app.store("settings.json") {
                    store.get("silentStart")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    false
                }
            };

            // 检查是否自动开启监控
            let auto_monitor = {
                if let Ok(store) = app.store("settings.json") {
                    store.get("autoMonitor")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    false
                }
            };

            // 检查命令行参数是否包含 --minimized
            let args: Vec<String> = std::env::args().collect();
            let is_autostart = args.iter().any(|a| a == "--minimized");

            // 如果是自启动且开启了静默启动，隐藏窗口
            if is_autostart && silent_start {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            // 如果是自启动且开启了自动监控，启动监控
            if is_autostart && auto_monitor {
                MONITOR_RUNNING.store(true, Ordering::Relaxed);
                log::info!("自动开启监控模式");
            }

            // 创建托盘菜单
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("ACE Limiter")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            log::info!("ACE Limiter 已启动");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
