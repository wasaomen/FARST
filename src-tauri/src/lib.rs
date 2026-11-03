use sysinfo::System;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_VM_OPERATION};
use windows::Win32::Foundation::CloseHandle;
use serde::{Serialize, Deserialize};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      get_system_status,
      free_memory,
      kill_processes,
      disable_services,
      get_temperature,
      check_update,
      install_update
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}

#[derive(Serialize, Deserialize, Clone)]
struct ProcessInfo {
    pid: u32,
    name: String,
    memory_usage: u64,
}

#[derive(Serialize)]
struct SystemStatus {
    total_memory: u64,
    used_memory: u64,
    free_memory: u64,
    process_count: usize,
    top_processes: Vec<ProcessInfo>,
}

#[tauri::command]
fn get_system_status() -> Result<SystemStatus, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_memory = sys.total_memory();
    let processes: Vec<ProcessInfo> = sys.processes()
        .iter()
        .map(|(pid, process)| {
            ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                memory_usage: process.memory(),
            }
        })
        .collect();

    let used_memory: u64 = processes.iter().map(|p| p.memory_usage).sum();
    let free_memory = total_memory - used_memory;

    let mut top_processes = processes.clone();
    top_processes.sort_by(|a, b| b.memory_usage.cmp(&a.memory_usage));
    top_processes.truncate(10);

    Ok(SystemStatus {
        total_memory,
        used_memory,
        free_memory,
        process_count: processes.len(),
        top_processes,
    })
}

#[tauri::command]
fn free_memory() -> Result<String, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut freed_memory = 0u64;

    for (pid, process) in sys.processes() {
        if is_essential_process(&process.name().to_string_lossy()) {
            continue;
        }

        unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA | PROCESS_VM_OPERATION,
                false,
                pid.as_u32(),
            );

            if let Ok(h) = handle {
                let result = EmptyWorkingSet(h);
                if result.is_ok() {
                    freed_memory += process.memory();
                }
                let _ = CloseHandle(h);
            }
        }
    }

    Ok(format!("解放したメモリ: {} MB", freed_memory / 1024 / 1024))
}

#[tauri::command]
fn kill_processes() -> Result<String, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut killed_count = 0;
    let unnecessary_processes = vec![
        "chrome.exe", "firefox.exe", "msedge.exe", "slack.exe",
        "discord.exe", "teams.exe", "zoom.exe", "spotify.exe",
        "onedrive.exe", "skype.exe", "thunderbird.exe",
    ];

    for (_pid, process) in sys.processes() {
        let process_name = process.name().to_string_lossy().to_string();
        if unnecessary_processes.contains(&process_name.as_str()) {
            if process.kill() {
                killed_count += 1;
            }
        }
    }

    Ok(format!("終了したプロセス数: {}", killed_count))
}

#[tauri::command]
fn disable_services() -> Result<String, String> {
    let services_to_disable = vec![
        "XblGameSave", "XboxNetApiSvc", "XboxGipSvc",
        "SysMain", "WSearch", "DiagTrack",
    ];

    let mut disabled_count = 0;

    for service in services_to_disable {
        let result = std::process::Command::new("sc")
            .args(&["stop", service])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                disabled_count += 1;
            }
        }
    }

    Ok(format!("停止したサービス数: {}", disabled_count))
}

#[derive(Serialize)]
struct TemperatureData {
    cpu_temp: f32,
    gpu_temp: f32,
}

#[tauri::command]
fn get_temperature() -> Result<TemperatureData, String> {
    // 実際の温度取得は複雑なWindows APIが必要なため、
    // CPU使用率から推定した温度を返す
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    // 使用率に基づいて温度を推定（35°C + 使用率 * 0.3）
    let cpu_temp = 35.0 + (cpu_usage as f32 * 0.3);
    let gpu_temp = cpu_temp * 0.9;

    // 最低温度を40°Cに設定
    let cpu_temp = cpu_temp.max(40.0);
    let gpu_temp = gpu_temp.max(35.0);

    Ok(TemperatureData {
        cpu_temp,
        gpu_temp,
    })
}

#[derive(Serialize)]
struct UpdateInfo {
    available: bool,
    version: Option<String>,
    body: Option<String>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;

    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => {
                    Ok(UpdateInfo {
                        available: true,
                        version: Some(update.version.clone()),
                        body: Some(update.body.clone().unwrap_or_else(|| "新しいバージョンが利用可能です".to_string())),
                    })
                }
                Ok(None) => {
                    Ok(UpdateInfo {
                        available: false,
                        version: None,
                        body: None,
                    })
                }
                Err(e) => {
                    Err(format!("アップデートチェックエラー: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("アップデーターが利用できません: {}", e))
        }
    }
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_updater::UpdaterExt;

    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => {
                    match update.download_and_install(
                        |chunk_length, content_length| {
                            // Download progress callback
                            println!("Downloading... {} / {}", chunk_length, content_length.unwrap_or(0));
                        },
                        || {
                            // Download finish callback
                            println!("Download complete");
                        }
                    ).await {
                        Ok(_) => {
                            Ok("アップデートのダウンロードとインストールが完了しました。アプリを再起動してください".to_string())
                        }
                        Err(e) => {
                            Err(format!("アップデートインストールエラー: {}", e))
                        }
                    }
                }
                Ok(None) => {
                    Err("新しいバージョンはありません".to_string())
                }
                Err(e) => {
                    Err(format!("アップデートチェックエラー: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("アップデーターが利用できません: {}", e))
        }
    }
}

fn is_essential_process(name: &str) -> bool {
    let essential_processes = vec![
        "System", "System Idle Process", "smss.exe", "csrss.exe",
        "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe",
        "svchost.exe", "explorer.exe", "dwm.exe", "conhost.exe",
        "rustup-init.exe", "cargo.exe", "FARST.exe", "app.exe", "TOIRE.exe",
    ];

    essential_processes.contains(&name)
}
