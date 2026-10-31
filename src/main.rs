use sysinfo::System;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_VM_OPERATION};
use windows::Win32::Foundation::CloseHandle;

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: u32,
    name: String,
    memory_usage: u64,
}

fn get_system_info() -> (Vec<ProcessInfo>, u64) {
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

    (processes, total_memory)
}

fn is_essential_process(name: &str) -> bool {
    let essential_processes = vec![
        "System", "System Idle Process", "smss.exe", "csrss.exe",
        "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe",
        "svchost.exe", "explorer.exe", "dwm.exe", "conhost.exe",
        "rustup-init.exe", "cargo.exe", "TOIRE.exe",
    ];

    essential_processes.contains(&name)
}

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

fn kill_unnecessary_processes() -> Result<String, String> {
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

fn print_system_status() {
    let (processes, total_memory) = get_system_info();

    println!("=== システムステータス ===");
    println!("総メモリ: {} MB", total_memory / 1024 / 1024);

    let used_memory: u64 = processes.iter().map(|p| p.memory_usage).sum();
    println!("使用メモリ: {} MB", used_memory / 1024 / 1024);
    println!("空きメモリ: {} MB", (total_memory - used_memory) / 1024 / 1024);
    println!("プロセス数: {}", processes.len());
    println!();

    println!("=== メモリ使用量の多いプロセス (トップ10) ===");
    let mut sorted_processes = processes.clone();
    sorted_processes.sort_by(|a, b| b.memory_usage.cmp(&a.memory_usage));

    for (i, process) in sorted_processes.iter().take(10).enumerate() {
        println!("{}. {} - {} MB (PID: {})", 
            i + 1, 
            process.name, 
            process.memory_usage / 1024 / 1024,
            process.pid
        );
    }
}

fn main() {
    println!("=== TOIRE - ゲーマー特化軽量化ツール ===\n");

    loop {
        println!("メニュー:");
        println!("1. システムステータス表示");
        println!("2. メモリ解放");
        println!("3. 不要プロセス終了");
        println!("4. サービス無効化");
        println!("5. 全部実行");
        println!("0. 終了");
        print!("選択: ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("入力エラー");

        match input.trim() {
            "1" => {
                print_system_status();
            }
            "2" => {
                match free_memory() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
            }
            "3" => {
                match kill_unnecessary_processes() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
            }
            "4" => {
                match disable_services() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
            }
            "5" => {
                println!("\n=== 全部実行中 ===\n");
                
                match free_memory() {
                    Ok(msg) => println!("✓ {}", msg),
                    Err(e) => println!("✗ メモリ解放エラー: {}", e),
                }

                match kill_unnecessary_processes() {
                    Ok(msg) => println!("✓ {}", msg),
                    Err(e) => println!("✗ プロセス終了エラー: {}", e),
                }

                match disable_services() {
                    Ok(msg) => println!("✓ {}", msg),
                    Err(e) => println!("✗ サービス無効化エラー: {}", e),
                }

                println!("\n完了!\n");
            }
            "0" => {
                println!("終了します。");
                break;
            }
            _ => {
                println!("無効な選択です。\n");
            }
        }
    }
}
