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
        "rustup-init.exe", "cargo.exe", "TOIRE.exe", "Zero.exe",
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

fn run_cmd(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn reg_add(key: &str, name: &str, value: &str, kind: &str) -> bool {
    run_cmd("reg", &[
        "add", key,
        "/v", name,
        "/t", kind,
        "/d", value,
        "/f",
    ])
}

fn interface_keys() -> Vec<String> {
    let output = std::process::Command::new("reg")
        .args(["query", r"HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces"])
        .output();

    match output {
        Ok(o) => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|line| {
                    !line.chars().next().map(|c| c.is_whitespace()).unwrap_or(true)
                        && line.contains("Interfaces\\")
                        && line.contains('{')
                })
                .map(|line| line.trim().to_string())
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

fn optimize_network() -> Result<String, String> {
    let mut applied = 0u32;

    // マルチメディアプロファイル: ネットワークスロットリング無効化 + ゲーム優先度
    let mm = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
    if reg_add(mm, "NetworkThrottlingIndex", "0xffffffff", "REG_DWORD") { applied += 1; }
    if reg_add(mm, "SystemResponsiveness", "0", "REG_DWORD") { applied += 1; }

    let games = format!(r"{}\Tasks\Games", mm);
    for (name, value, kind) in [
        ("GPU Priority", "8", "REG_DWORD"),
        ("Priority", "6", "REG_DWORD"),
        ("Scheduling Category", "High", "REG_SZ"),
        ("SFIO Priority", "High", "REG_SZ"),
    ] {
        if reg_add(&games, name, value, kind) { applied += 1; }
    }

    // 各ネットワークインターフェース: Nagle無効化 + ACK遅延無効化
    for key in interface_keys() {
        for (name, value) in [
            ("TcpAckFrequency", "1"),
            ("TCPNoDelay", "1"),
            ("TCPDelAckTicks", "0"),
        ] {
            if reg_add(&key, name, value, "REG_DWORD") { applied += 1; }
        }
    }

    // netsh TCPグローバル設定
    for args in [
        &["interface", "tcp", "set", "global", "autotuninglevel=high"][..],
        &["interface", "tcp", "set", "global", "ecncapability=disabled"][..],
        &["interface", "tcp", "set", "global", "timestamps=disabled"][..],
        &["interface", "tcp", "set", "global", "fastopen=enabled"][..],
    ] {
        if run_cmd("netsh", args) { applied += 1; }
    }

    run_cmd("ipconfig", &["/flushdns"]);

    Ok(format!(
        "ネットワーク最適化を適用しました ({}項目)。効果を得るにはネットワークの再接続またはPC再起動を推奨します。",
        applied
    ))
}

fn network_boost() -> Result<String, String> {
    let mut applied = 0u32;

    // CTCP輻輳プロバイダ: ロス・高遅延回線で安定したスループット
    if run_cmd("netsh", &["interface", "tcp", "set", "global", "congestionprovider=ctcp"]) {
        applied += 1;
    }

    // Game Mode を強制有効化
    if reg_add(r"HKCU\Software\Microsoft\GameBar", "AutoGameModeEnabled", "1", "REG_DWORD") { applied += 1; }
    if reg_add(r"HKCU\Software\Microsoft\GameBar", "AllowAutoGameMode", "1", "REG_DWORD") { applied += 1; }

    // QoS: 非最優先トラフィックの帯域制限を解除
    let mm = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
    if reg_add(mm, "NonBestEffortLimit", "0", "REG_DWORD") { applied += 1; }

    // Delivery Optimization (Windows Update P2P) を停止: 上り回線の圧迫を防ぐ
    if run_cmd("sc", &["stop", "DoSvc"]) { applied += 1; }
    if run_cmd("sc", &["config", "DoSvc", "start=", "disabled"]) { applied += 1; }

    // NIC省電力機能・割り込み緩和を無効化してレイテンシのブレを低減
    let ps = concat!(
        "Get-NetAdapter -ErrorAction SilentlyContinue | ",
        "Get-NetAdapterAdvancedProperty -ErrorAction SilentlyContinue | ",
        "Where-Object { $_.DisplayName -match 'Energy Efficient Ethernet|Green Ethernet|Power Saving|Interrupt Moderation' } | ",
        "ForEach-Object { try { Set-NetAdapterAdvancedProperty -Name $_.Name -DisplayName $_.DisplayName -DisplayValue 'Disabled' -ErrorAction Stop } catch {} }"
    );
    if run_cmd("powershell", &["-NoProfile", "-Command", ps]) { applied += 1; }

    // ネットワークアダプタの省電力停止を禁止
    let ps2 = "Get-NetAdapter -ErrorAction SilentlyContinue | Set-NetAdapterPowerManagement -AllowTurnOffDevice Disabled -ErrorAction SilentlyContinue";
    if run_cmd("powershell", &["-NoProfile", "-Command", ps2]) { applied += 1; }

    run_cmd("ipconfig", &["/flushdns"]);

    Ok(format!(
        "回線ブーストを適用しました ({}項目)。悪い回線でのping安定化に効果があります。ネットワーク再接続またはPC再起動を推奨します。",
        applied
    ))
}

fn extract_ms_values(line: &str) -> Vec<f64> {
    let lower = line.to_lowercase();
    let bytes = lower.as_bytes();
    let mut values = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'm' && bytes[i + 1] == b's' {
            let mut j = i;
            while j > 0 && matches!(bytes[j - 1], b'0'..=b'9' | b'=' | b'<' | b' ' | b'\t') {
                j -= 1;
            }
            let digits: String = lower[j..i].chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                if let Ok(v) = digits.parse::<f64>() {
                    values.push(v);
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    values
}

fn ping_test() -> Result<String, String> {
    let output = std::process::Command::new("ping")
        .args(["-n", "4", "1.1.1.1"])
        .output()
        .map_err(|e| format!("ping実行エラー: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut values = Vec::new();

    for line in text.lines() {
        let lower = line.to_lowercase();
        if !lower.contains("ms") {
            continue;
        }
        if lower.contains("min") || lower.contains("max") || lower.contains("avg")
            || lower.contains("平均") || lower.contains("最小") || lower.contains("最大") {
            continue;
        }
        values.extend(extract_ms_values(line));
    }

    if values.is_empty() {
        return Err("pingの応答を解析できませんでした".to_string());
    }

    let avg = values.iter().sum::<f64>() / values.len() as f64;
    Ok(format!("平均ping: {:.0} ms ({}回応答)", avg, values.len()))
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
    println!("=== Zero - ゲーマー特化軽量化ツール ===\n");

    loop {
        println!("メニュー:");
        println!("1. システムステータス表示");
        println!("2. メモリ解放");
        println!("3. 不要プロセス終了");
        println!("4. サービス無効化");
        println!("5. 全部実行");
        println!("6. ネットワーク最適化 (レイテンシ低減)");
        println!("7. pingテスト");
        println!("8. 回線ブースト (悪い回線向け)");
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

                match optimize_network() {
                    Ok(msg) => println!("✓ {}", msg),
                    Err(e) => println!("✗ ネットワーク最適化エラー: {}", e),
                }

                match network_boost() {
                    Ok(msg) => println!("✓ {}", msg),
                    Err(e) => println!("✗ 回線ブーストエラー: {}", e),
                }

                println!("\n完了!\n");
            }
            "6" => {
                match optimize_network() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
            }
            "7" => {
                match ping_test() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
            }
            "8" => {
                match network_boost() {
                    Ok(msg) => println!("\n✓ {}\n", msg),
                    Err(e) => println!("\n✗ エラー: {}\n", e),
                }
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
