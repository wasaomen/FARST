use sysinfo::System;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_VM_OPERATION};
use windows::Win32::Foundation::CloseHandle;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tauri::Manager;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
      optimize_network,
      network_boost,
      ping_test,
      check_update,
      install_update,
      github_login_start,
      github_login_poll,
      github_status,
      github_logout,
      github_open_url,
      settings_get,
      settings_set,
      settings_sync_push,
      settings_sync_pull,
      create_restore_point,
      list_restore_points,
      flush_dns_cache,
      repair_network,
      repair_system_files,
      repair_disk,
      clean_temp_files,
      apply_tweak,
      apply_game_profile,
      get_startup_apps,
      set_startup_app,
      get_system_info,
      get_process_list,
      detect_games,
      clean_disk_deep,
      registry_clean,
      restore_tweak,
      win_minimize,
      win_close,
      win_toggle_maximize,
      win_drag,
      discord_login_start,
      discord_login_poll,
      discord_status,
      discord_logout,
      discord_server_info,
      license_activate,
      license_status,
      license_deactivate
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
fn free_memory(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
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
fn kill_processes(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
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
fn disable_services(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    let services_to_disable = vec![
        "XblGameSave", "XboxNetApiSvc", "XboxGipSvc",
        "SysMain", "WSearch", "DiagTrack",
    ];

    let mut disabled_count = 0;

    for service in services_to_disable {
        let result = hidden_console(std::process::Command::new("sc"))
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

// コンソールウィンドウを表示せずにコマンドを実行するためのフラグ
fn hidden_console(mut cmd: std::process::Command) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

fn run_cmd(program: &str, args: &[&str]) -> bool {
    hidden_console(std::process::Command::new(program))
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
    let output = hidden_console(std::process::Command::new("reg"))
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

#[tauri::command]
fn optimize_network(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
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
        "ネットワーク最適化を適用しました ({}項目)。レイテンシ低減の効果を得るにはネットワークの再接続またはPC再起動を推奨します。",
        applied
    ))
}

#[tauri::command]
fn network_boost(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
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

#[derive(Serialize)]
struct PingResult {
    avg_ms: f64,
    replies: usize,
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

#[tauri::command]
fn ping_test() -> Result<PingResult, String> {
    let output = hidden_console(std::process::Command::new("ping"))
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
        // 集計行（最小/最大/平均）は除外
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
    Ok(PingResult {
        avg_ms: avg,
        replies: values.len(),
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

// ===================== GitHub連携 / 設定同期 =====================

// GitHub OAuth App の Client ID（デバイスフロー用。シークレットは不要）
// https://github.com/settings/developers でOAuth Appを作成して設定すること
const GITHUB_CLIENT_ID: &str = "Ov23lihk8weUr2EV6HXk";
const GIST_DESCRIPTION: &str = "Zero Settings";
const GIST_FILENAME: &str = "settings.json";

#[derive(Serialize, Deserialize, Clone)]
struct AuthData {
    access_token: String,
    login: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

fn app_file_path(app: &tauri::AppHandle, name: &str) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("app_data_dir error: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("dir create error: {}", e))?;
    Ok(dir.join(name))
}

fn load_auth(app: &tauri::AppHandle) -> Option<AuthData> {
    let path = app_file_path(app, "auth.json").ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_auth(app: &tauri::AppHandle, auth: &AuthData) -> Result<(), String> {
    let path = app_file_path(app, "auth.json")?;
    let content = serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| format!("auth save error: {}", e))
}

fn github_api_request(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Result<Value, String> {
    let url = format!("https://api.github.com{}", path);

    macro_rules! auth_headers {
        ($req:expr) => {
            $req
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", "Zero-System-Optimizer")
        };
    }
    macro_rules! with_auth {
        ($req:expr) => {
            match token {
                Some(t) => $req.header("Authorization", &format!("Bearer {}", t)),
                None => $req,
            }
        };
    }

    let result = match (method, body) {
        ("GET", None) | ("DELETE", None) => {
            let req = if method == "GET" { ureq::get(&url) } else { ureq::delete(&url) };
            let req = with_auth!(auth_headers!(req));
            req.call()
        }
        ("POST", b) | ("PATCH", b) => {
            let req = if method == "POST" { ureq::post(&url) } else { ureq::patch(&url) };
            let req = with_auth!(auth_headers!(req));
            match b {
                Some(v) => req.send_json(v),
                None => req.send_empty(),
            }
        }
        _ => return Err(format!("unsupported request: {}", method)),
    };

    match result {
        Ok(mut resp) => {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            if text.is_empty() {
                Ok(json!({}))
            } else {
                serde_json::from_str(&text).map_err(|e| format!("json parse error: {}", e))
            }
        }
        Err(ureq::Error::StatusCode(code)) => {
            Err(format!("GitHub API error: HTTP {}", code))
        }
        Err(e) => Err(format!("request error: {}", e)),
    }
}

fn github_post_form(url: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    let result = ureq::post(url)
        .header("Accept", "application/json")
        .header("User-Agent", "Zero-System-Optimizer")
        .send_form(params.iter().copied());
    match result {
        Ok(mut resp) => {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            serde_json::from_str(&text).map_err(|e| format!("json parse error: {}", e))
        }
        Err(ureq::Error::StatusCode(code)) => Err(format!("HTTP {}", code)),
        Err(e) => Err(format!("request error: {}", e)),
    }
}

#[derive(Serialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[tauri::command]
fn github_login_start() -> Result<DeviceCodeResponse, String> {
    let res = github_post_form("https://github.com/login/device/code", &[
        ("client_id", GITHUB_CLIENT_ID),
        ("scope", "gist"),
    ])?;
    Ok(DeviceCodeResponse {
        device_code: res["device_code"].as_str().unwrap_or_default().to_string(),
        user_code: res["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: res["verification_uri"].as_str().unwrap_or_default().to_string(),
        expires_in: res["expires_in"].as_u64().unwrap_or(900),
        interval: res["interval"].as_u64().unwrap_or(5),
    })
}

#[derive(Serialize)]
struct LoginPollResult {
    status: String, // "pending" | "success" | "error"
    login: Option<String>,
    message: Option<String>,
}

#[tauri::command]
fn github_login_poll(app: tauri::AppHandle, device_code: String) -> Result<LoginPollResult, String> {
    let res = github_post_form("https://github.com/login/oauth/access_token", &[
        ("client_id", GITHUB_CLIENT_ID),
        ("device_code", &device_code),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ])?;

    if let Some(token) = res["access_token"].as_str() {
        // ユーザー名を取得して保存
        let user = github_api_request("GET", "/user", Some(token), None)
            .unwrap_or_else(|_| json!({}));
        let login = user["login"].as_str().unwrap_or("unknown").to_string();
        let auth = AuthData {
            access_token: token.to_string(),
            login: login.clone(),
            avatar_url: None,
        };
        save_auth(&app, &auth)?;
        return Ok(LoginPollResult {
            status: "success".to_string(),
            login: Some(login),
            message: None,
        });
    }

    let error = res["error"].as_str().unwrap_or("unknown");
    match error {
        "authorization_pending" => Ok(LoginPollResult {
            status: "pending".to_string(),
            login: None,
            message: None,
        }),
        "slow_down" => Ok(LoginPollResult {
            status: "pending".to_string(),
            login: None,
            message: Some("slow_down".to_string()),
        }),
        _ => Ok(LoginPollResult {
            status: "error".to_string(),
            login: None,
            message: Some(error.to_string()),
        }),
    }
}

#[derive(Serialize)]
struct GithubStatus {
    logged_in: bool,
    login: Option<String>,
}

#[tauri::command]
fn github_status(app: tauri::AppHandle) -> Result<GithubStatus, String> {
    match load_auth(&app) {
        Some(auth) => Ok(GithubStatus {
            logged_in: true,
            login: Some(auth.login),
        }),
        None => Ok(GithubStatus {
            logged_in: false,
            login: None,
        }),
    }
}

#[tauri::command]
fn github_logout(app: tauri::AppHandle) -> Result<(), String> {
    let path = app_file_path(&app, "auth.json")?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("delete error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn github_open_url(url: String) -> Result<(), String> {
    // WindowsのデフォルトブラウザでURLを開く
    // rundll32 url.dll,FileProtocolHandler はシェルを介さないため & を含むURLも安全
    hidden_console(std::process::Command::new("rundll32"))
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map_err(|e| format!("open url error: {}", e))?;
    Ok(())
}

// ===================== Discord連携 =====================

// Discord Developer Portal でアプリを作成し、OAuth2 の Client ID / Secret と
// Redirects に http://localhost:53682/callback を登録すること
const DISCORD_CLIENT_ID: &str = "1566829612137451581";
const DISCORD_CLIENT_SECRET: &str = "1hTDwpzFiAZ7BI5vRJqMA55iRBhdPnK5";
const DISCORD_REDIRECT_URI: &str = "http://127.0.0.1:53682/callback";
static DISCORD_LOGIN_RESULT: Mutex<Option<Result<(String, bool), String>>> = Mutex::new(None);
static DISCORD_EXPECTED_STATE: Mutex<Option<String>> = Mutex::new(None);
static DISCORD_LISTENER: Mutex<Option<Arc<TcpListener>>> = Mutex::new(None);

fn set_discord_result(r: Result<(String, bool), String>) {
    if let Ok(mut g) = DISCORD_LOGIN_RESULT.lock() {
        *g = Some(r);
    }
}

fn discord_auth_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app_file_path(app, "discord_auth.json")
}

fn load_discord_auth(app: &tauri::AppHandle) -> Option<AuthData> {
    let path = discord_auth_path(app).ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_discord_auth(app: &tauri::AppHandle, auth: &AuthData) -> Result<(), String> {
    let path = discord_auth_path(app)?;
    let content = serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| format!("discord auth save error: {}", e))
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn write_http_response(stream: &mut std::net::TcpStream, title: &str, msg: &str) -> std::io::Result<()> {
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{0}</title><style>body{{font-family:sans-serif;background:#111;color:#eee;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}div{{text-align:center}}h1{{color:#3b82f6}}</style></head><body><div><h1>{0}</h1><p>{1}</p><p style=\"color:#888\">このタブは閉じて構いません。</p></div></body></html>",
        title, msg
    );
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    stream.write_all(resp.as_bytes())
}

fn discord_avatar_url(user: &Value) -> Option<String> {
    let id = user["id"].as_str()?;
    if let Some(hash) = user["avatar"].as_str() {
        if !hash.is_empty() {
            return Some(format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=128", id, hash));
        }
    }
    // アバター未設定時はデフォルトアバター
    let idx = (id.parse::<u64>().ok()? >> 22) % 6;
    Some(format!("https://cdn.discordapp.com/embed/avatars/{}.png", idx))
}

fn discord_exchange_code(app: &tauri::AppHandle, code: &str) -> Result<(String, bool), String> {
    let mut res = ureq::post("https://discord.com/api/oauth2/token")
        .send_form([
            ("client_id", DISCORD_CLIENT_ID),
            ("client_secret", DISCORD_CLIENT_SECRET),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", DISCORD_REDIRECT_URI),
        ].iter().copied())
        .map_err(|e| format!("token request error: {}", e))?;
    let text = res.body_mut().read_to_string().unwrap_or_default();
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("token parse error: {} (body: {})", e, &text[..text.len().min(300)]))?;
    if let Some(err) = json["error"].as_str() {
        let desc = json["error_description"].as_str().unwrap_or("");
        let msg = json["message"].as_str().unwrap_or("");
        return Err(format!("discord error: {} {} {}", err, desc, msg).trim().to_string());
    }
    let token = json["access_token"].as_str()
        .ok_or_else(|| "access_token がありません".to_string())?
        .to_string();

    let mut user_res = ureq::get("https://discord.com/api/users/@me")
        .header("Authorization", &format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("user request error: {}", e))?;
    let user_text = user_res.body_mut().read_to_string().unwrap_or_default();
    let user: Value = serde_json::from_str(&user_text).map_err(|e| format!("user parse error: {}", e))?;
    let username = user["global_name"].as_str()
        .or_else(|| user["username"].as_str())
        .unwrap_or("unknown")
        .to_string();

    let avatar_url = discord_avatar_url(&user);

    save_discord_auth(app, &AuthData {
        access_token: token.clone(),
        login: username.clone(),
        avatar_url,
    })?;

    Ok((username, false))
}

fn bind_discord_listener() -> Result<TcpListener, String> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("socket error: {}", e))?;
    socket.set_reuse_address(true).map_err(|e| format!("socket error: {}", e))?;
    socket.set_nonblocking(true).map_err(|e| format!("socket error: {}", e))?;
    let addr: std::net::SocketAddr = "127.0.0.1:53682".parse().map_err(|e| format!("addr error: {}", e))?;
    socket.bind(&addr.into())
        .map_err(|e| format!("callback port bind error: {}", e))?;
    socket.listen(16).map_err(|e| format!("listener error: {}", e))?;
    Ok(TcpListener::from(socket))
}

fn discord_accept_loop(listener: Arc<TcpListener>, app: tauri::AppHandle) {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.lines().next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("");
                let query = path.split('?').nth(1).unwrap_or("");

                let mut code = None;
                let mut got_state = None;
                let mut oauth_error = None;
                for pair in query.split('&') {
                    let mut it = pair.splitn(2, '=');
                    match (it.next(), it.next()) {
                        (Some("code"), Some(v)) => code = Some(v.to_string()),
                        (Some("state"), Some(v)) => got_state = Some(v.to_string()),
                        (Some("error"), Some(v)) => oauth_error = Some(v.to_string()),
                        _ => {}
                    }
                }

                if let Some(err) = oauth_error {
                    let _ = write_http_response(&mut stream, "ログインキャンセル", &format!("Discord側でキャンセルされました: {}", err));
                    set_discord_result(Err(format!("キャンセルされました: {}", err)));
                    continue;
                }

                let expected = DISCORD_EXPECTED_STATE.lock().ok().and_then(|g| g.clone());
                match (code, got_state) {
                    (Some(c), Some(s)) if expected.as_deref() == Some(s.as_str()) => {
                        let result = discord_exchange_code(&app, &c);
                        let (title, msg) = match &result {
                            Ok((u, joined)) => (
                                "ログイン完了",
                                if *joined {
                                    format!("Zeroへのログインが完了しました: {}（サーバーに参加しました）", u)
                                } else {
                                    format!("Zeroへのログインが完了しました: {}", u)
                                },
                            ),
                            Err(e) => ("ログイン失敗", format!("エラー: {}", e)),
                        };
                        let _ = write_http_response(&mut stream, title, &msg);
                        set_discord_result(result);
                        if let Ok(mut g) = DISCORD_EXPECTED_STATE.lock() {
                            *g = None;
                        }
                    }
                    _ => {
                        let _ = write_http_response(&mut stream, "エラー", "stateが一致しません");
                    }
                }
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

#[tauri::command]
fn discord_login_start(app: tauri::AppHandle) -> Result<String, String> {
    if DISCORD_CLIENT_ID == "YOUR_DISCORD_CLIENT_ID" || DISCORD_CLIENT_SECRET == "YOUR_DISCORD_CLIENT_SECRET" {
        return Err("Discord Client ID/Secret が未設定です".to_string());
    }
    set_discord_result_pending();

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let state = format!("{:x}{:x}", std::process::id(), nanos);
    if let Ok(mut g) = DISCORD_EXPECTED_STATE.lock() {
        *g = Some(state.clone());
    }

    // コールバックリスナーは常駐（初回のみ bind）
    {
        let mut guard = DISCORD_LISTENER.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            let listener = bind_discord_listener()?;
            let arc = Arc::new(listener);
            let app_handle = app.clone();
            let loop_listener = arc.clone();
            std::thread::spawn(move || {
                discord_accept_loop(loop_listener, app_handle);
            });
            *guard = Some(arc);
        }
    }

    Ok(format!(
        "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        DISCORD_CLIENT_ID,
        url_encode(DISCORD_REDIRECT_URI),
        url_encode("identify"),
        state
    ))
}

fn set_discord_result_pending() {
    if let Ok(mut g) = DISCORD_LOGIN_RESULT.lock() {
        *g = None;
    }
}

#[derive(Serialize)]
struct DiscordPollResult {
    status: String, // "pending" | "success" | "error"
    login: Option<String>,
    message: Option<String>,
    joined: Option<bool>,
}

#[tauri::command]
fn discord_login_poll() -> Result<DiscordPollResult, String> {
    let guard = DISCORD_LOGIN_RESULT.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        None => Ok(DiscordPollResult {
            status: "pending".to_string(),
            login: None,
            message: None,
            joined: None,
        }),
        Some(Ok((u, joined))) => Ok(DiscordPollResult {
            status: "success".to_string(),
            login: Some(u.clone()),
            message: None,
            joined: Some(*joined),
        }),
        Some(Err(e)) => Ok(DiscordPollResult {
            status: "error".to_string(),
            login: None,
            message: Some(e.clone()),
            joined: None,
        }),
    }
}

#[derive(Serialize)]
struct DiscordStatus {
    logged_in: bool,
    login: Option<String>,
    avatar_url: Option<String>,
}

#[tauri::command]
fn discord_status(app: tauri::AppHandle) -> Result<DiscordStatus, String> {
    match load_discord_auth(&app) {
        Some(auth) => Ok(DiscordStatus {
            logged_in: true,
            login: Some(auth.login),
            avatar_url: auth.avatar_url,
        }),
        None => Ok(DiscordStatus {
            logged_in: false,
            login: None,
            avatar_url: None,
        }),
    }
}

#[tauri::command]
fn discord_logout(app: tauri::AppHandle) -> Result<(), String> {
    let path = discord_auth_path(&app)?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("delete error: {}", e))?;
    }
    Ok(())
}

#[derive(Serialize)]
struct DiscordServerInfo {
    ok: bool,
    name: Option<String>,
    member_count: Option<u64>,
    online_count: Option<u64>,
    icon_url: Option<String>,
}

#[tauri::command]
fn discord_server_info() -> Result<DiscordServerInfo, String> {
    let res = ureq::get("https://discord.com/api/v10/invites/yHrB7v66UV?with_counts=true")
        .call()
        .map_err(|e| format!("invite request error: {}", e))?;
    let mut resp = res;
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("invite parse error: {}", e))?;
    let guild = &json["guild"];
    let icon_url = guild["id"].as_str().and_then(|gid| {
        guild["icon"].as_str().and_then(|hash| {
            if hash.is_empty() {
                None
            } else {
                let ext = if hash.starts_with("a_") { "gif" } else { "png" };
                Some(format!("https://cdn.discordapp.com/icons/{}/{}.{}?size=128", gid, hash, ext))
            }
        })
    });
    Ok(DiscordServerInfo {
        ok: true,
        name: guild["name"].as_str().map(|s| s.to_string()),
        member_count: guild["approximate_member_count"].as_u64(),
        online_count: guild["approximate_presence_count"].as_u64(),
        icon_url,
    })
}

// ===================== ライセンス（Lemon Squeezy） =====================
const LS_API_BASE: &str = "https://api.lemonsqueezy.com/v1";
const LICENSE_OFFLINE_GRACE_SECS: u64 = 7 * 24 * 3600;
// 管理者用マスターキー（オフラインで常に有効・テスト用）
const ADMIN_LICENSE_KEY: &str = "admin978kir843u";

#[derive(Serialize, Deserialize, Clone)]
struct LicenseData {
    license_key: String,
    instance_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    last_validated: u64,
}

fn license_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app_file_path(app, "license.json")
}

fn load_license(app: &tauri::AppHandle) -> Option<LicenseData> {
    let path = license_path(app).ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_license(app: &tauri::AppHandle, data: &LicenseData) -> Result<(), String> {
    let path = license_path(app)?;
    let content = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| format!("license save error: {}", e))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_instance_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}", std::process::id(), nanos)
}

fn ls_license_request(endpoint: &str, license_key: &str, instance_id: &str) -> Result<Value, String> {
    let mut res = ureq::post(&format!("{}/licenses/{}", LS_API_BASE, endpoint))
        .header("Accept", "application/json")
        .send_json(json!({
            "license_key": license_key,
            "instance_id": instance_id,
        }))
        .map_err(|e| format!("license request error: {}", e))?;
    let text = res.body_mut().read_to_string().unwrap_or_default();
    serde_json::from_str(&text).map_err(|e| format!("license parse error: {}", e))
}

fn ls_license_status(json: &Value) -> String {
    json["license_key"]["status"].as_str()
        .or_else(|| json["status"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn ensure_premium(app: &tauri::AppHandle) -> Result<(), String> {
    let data = load_license(app)
        .ok_or_else(|| "Premiumライセンスが必要です".to_string())?;
    // 管理者キーは常に有効
    if data.license_key == ADMIN_LICENSE_KEY {
        return Ok(());
    }
    if data.status != "active" {
        return Err("Premiumライセンスが必要です".to_string());
    }
    if now_unix().saturating_sub(data.last_validated) > LICENSE_OFFLINE_GRACE_SECS {
        return Err("ライセンスの再確認が必要です。Accountページで状態を更新してください".to_string());
    }
    Ok(())
}

fn mask_license_key(key: &str) -> String {
    if key.len() <= 8 {
        return "••••".to_string();
    }
    format!("••••-••••-{}", &key[key.len() - 4..])
}

#[derive(Serialize)]
struct LicenseStatus {
    activated: bool,
    status: String,
    key_masked: Option<String>,
    last_validated: u64,
}

#[tauri::command]
fn license_activate(app: tauri::AppHandle, key: String) -> Result<LicenseStatus, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("ライセンスキーを入力してください".to_string());
    }
    let mut data = load_license(&app).unwrap_or(LicenseData {
        license_key: String::new(),
        instance_id: new_instance_id(),
        status: String::new(),
        last_validated: 0,
    });
    if data.instance_id.is_empty() {
        data.instance_id = new_instance_id();
    }
    data.license_key = key;

    // 管理者キーはAPI不要で即有効
    if data.license_key == ADMIN_LICENSE_KEY {
        data.status = "active".to_string();
        data.last_validated = now_unix();
        save_license(&app, &data)?;
        return Ok(LicenseStatus {
            activated: true,
            status: data.status.clone(),
            key_masked: Some(mask_license_key(&data.license_key)),
            last_validated: data.last_validated,
        });
    }

    let json = ls_license_request("activate", &data.license_key, &data.instance_id)?;
    let activated = json["activated"].as_bool().unwrap_or(false);
    if !activated {
        let err = json["error"].as_str().unwrap_or("activation failed");
        return Err(format!("ライセンスの有効化に失敗しました: {}", err));
    }
    data.status = ls_license_status(&json);
    data.last_validated = now_unix();
    save_license(&app, &data)?;

    Ok(LicenseStatus {
        activated: true,
        status: data.status.clone(),
        key_masked: Some(mask_license_key(&data.license_key)),
        last_validated: data.last_validated,
    })
}

#[tauri::command]
fn license_status(app: tauri::AppHandle) -> Result<LicenseStatus, String> {
    let Some(mut data) = load_license(&app) else {
        return Ok(LicenseStatus {
            activated: false,
            status: "none".to_string(),
            key_masked: None,
            last_validated: 0,
        });
    };
    if data.instance_id.is_empty() {
        data.instance_id = new_instance_id();
    }

    // 管理者キーは常に有効
    if data.license_key == ADMIN_LICENSE_KEY {
        data.status = "active".to_string();
        data.last_validated = now_unix();
        let _ = save_license(&app, &data);
        return Ok(LicenseStatus {
            activated: true,
            status: "active".to_string(),
            key_masked: Some(mask_license_key(&data.license_key)),
            last_validated: data.last_validated,
        });
    }

    match ls_license_request("validate", &data.license_key, &data.instance_id) {
        Ok(json) => {
            let valid = json["valid"].as_bool().unwrap_or(false);
            data.status = if valid { ls_license_status(&json) } else { "invalid".to_string() };
            if data.status.is_empty() || data.status == "unknown" {
                data.status = if valid { "active".to_string() } else { "invalid".to_string() };
            }
            data.last_validated = now_unix();
            let _ = save_license(&app, &data);
        }
        Err(_) => {
            // オフライン時は猶予期間内なら前回のステータスを維持
            if now_unix().saturating_sub(data.last_validated) > LICENSE_OFFLINE_GRACE_SECS {
                data.status = "unknown".to_string();
            }
        }
    }

    let activated = data.status == "active";
    Ok(LicenseStatus {
        activated,
        status: data.status.clone(),
        key_masked: Some(mask_license_key(&data.license_key)),
        last_validated: data.last_validated,
    })
}

#[tauri::command]
fn license_deactivate(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(data) = load_license(&app) {
        let _ = ls_license_request("deactivate", &data.license_key, &data.instance_id);
    }
    let path = license_path(&app)?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("delete error: {}", e))?;
    }
    Ok(())
}

fn find_settings_gist(token: &str) -> Result<Option<String>, String> {
    let gists = github_api_request("GET", "/gists?per_page=100", Some(token), None)?;
    if let Some(arr) = gists.as_array() {
        for gist in arr {
            if gist["description"].as_str() == Some(GIST_DESCRIPTION) {
                if let Some(id) = gist["id"].as_str() {
                    return Ok(Some(id.to_string()));
                }
            }
        }
    }
    Ok(None)
}

fn read_local_settings(app: &tauri::AppHandle) -> Result<String, String> {
    let path = app_file_path(app, "settings.json")?;
    if path.exists() {
        std::fs::read_to_string(path).map_err(|e| format!("settings read error: {}", e))
    } else {
        Ok("{}".to_string())
    }
}

fn write_local_settings(app: &tauri::AppHandle, content: &str) -> Result<(), String> {
    let path = app_file_path(app, "settings.json")?;
    std::fs::write(path, content).map_err(|e| format!("settings write error: {}", e))
}

#[tauri::command]
fn settings_get(app: tauri::AppHandle) -> Result<Value, String> {
    let content = read_local_settings(&app)?;
    serde_json::from_str(&content).map_err(|e| format!("settings parse error: {}", e))
}

#[tauri::command]
fn settings_set(app: tauri::AppHandle, settings: Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    write_local_settings(&app, &content)
}

#[tauri::command]
fn settings_sync_push(app: tauri::AppHandle) -> Result<String, String> {
    let auth = load_auth(&app).ok_or_else(|| "GitHubにログインしていません".to_string())?;
    let content = read_local_settings(&app)?;
    let body = json!({
        "description": GIST_DESCRIPTION,
        "public": false,
        "files": {
            GIST_FILENAME: { "content": content }
        }
    });

    match find_settings_gist(&auth.access_token)? {
        Some(gist_id) => {
            github_api_request("PATCH", &format!("/gists/{}", gist_id), Some(&auth.access_token), Some(body))?;
        }
        None => {
            github_api_request("POST", "/gists", Some(&auth.access_token), Some(body))?;
        }
    }
    Ok("設定をGitHub Gistに同期しました".to_string())
}

#[tauri::command]
fn settings_sync_pull(app: tauri::AppHandle) -> Result<Value, String> {
    let auth = load_auth(&app).ok_or_else(|| "GitHubにログインしていません".to_string())?;
    let gist_id = find_settings_gist(&auth.access_token)?
        .ok_or_else(|| "同期用Gistが見つかりません".to_string())?;
    let gist = github_api_request("GET", &format!("/gists/{}", gist_id), Some(&auth.access_token), None)?;
    let content = gist["files"][GIST_FILENAME]["content"]
        .as_str()
        .ok_or_else(|| "Gist内にsettings.jsonが見つかりません".to_string())?
        .to_string();
    write_local_settings(&app, &content)?;
    serde_json::from_str(&content).map_err(|e| format!("settings parse error: {}", e))
}

// ===================== 実用機能コマンド =====================

fn ps_output(script: &str) -> Result<String, String> {
    let out = hidden_console(std::process::Command::new("powershell"))
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("powershell error: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() && !stderr.trim().is_empty() {
        return Err(format!("{}", stderr.trim()));
    }
    Ok(stdout)
}

// --- System Restore ---

#[tauri::command]
fn create_restore_point(app: tauri::AppHandle, name: String) -> Result<String, String> {
    ensure_premium(&app)?;
    let safe = name.replace('\'', "''");
    let script = format!(
        "try {{ Checkpoint-Computer -Description '{}' -RestorePointType MODIFY_SETTINGS; 'OK' }} catch {{ Write-Error $_ }}",
        safe
    );
    let out = ps_output(&script)?;
    if out.contains("OK") {
        Ok("復元ポイントを作成しました".to_string())
    } else {
        Err(format!("復元ポイント作成失敗: {}", out.trim()))
    }
}

#[derive(Serialize)]
struct RestorePoint {
    sequence_number: u64,
    description: String,
    creation_time: String,
}

#[tauri::command]
fn list_restore_points() -> Result<Vec<RestorePoint>, String> {
    let script = "Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object SequenceNumber -Descending | Select-Object -First 10 | ForEach-Object { \"$($_.SequenceNumber)`t$($_.Description)`t$($_.CreationTime.ToString('yyyy-MM-dd HH:mm'))\" }";
    let out = ps_output(script)?;
    let points = out
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let seq = parts.next()?.trim().parse::<u64>().ok()?;
            let desc = parts.next()?.trim().to_string();
            let time = parts.next().unwrap_or("").trim().to_string();
            Some(RestorePoint {
                sequence_number: seq,
                description: desc,
                creation_time: time,
            })
        })
        .collect();
    Ok(points)
}

// --- Fixes ---

#[tauri::command]
fn flush_dns_cache(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    if run_cmd("ipconfig", &["/flushdns"]) {
        Ok("DNSキャッシュをクリアしました".to_string())
    } else {
        Err("DNSフラッシュに失敗しました".to_string())
    }
}

#[tauri::command]
fn repair_network(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    let mut results = Vec::new();
    if run_cmd("netsh", &["winsock", "reset"]) { results.push("Winsock reset"); }
    if run_cmd("netsh", &["int", "ip", "reset"]) { results.push("TCP/IP reset"); }
    run_cmd("ipconfig", &["/flushdns"]);
    Ok(format!("ネットワーク修復を実行しました ({})。再起動が必要な場合があります。", results.join(", ")))
}

#[tauri::command]
fn repair_system_files(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    // sfc /scannow は長時間かかるためバックグラウンドで起動
    std::process::Command::new("cmd")
        .args(["/c", "start", "/min", "", "sfc", "/scannow"])
        .spawn()
        .map_err(|e| format!("sfc起動エラー: {}", e))?;
    Ok("システムファイルの修復を開始しました。完了まで数分〜十数分かかります（管理者権限が必要です）".to_string())
}

#[tauri::command]
fn repair_disk(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    // オンラインスキャン (管理者権限で実行される)
    std::process::Command::new("cmd")
        .args(["/c", "start", "/min", "", "chkdsk", "C:", "/scan"])
        .spawn()
        .map_err(|e| format!("chkdsk起動エラー: {}", e))?;
    Ok("ディスクスキャンを開始しました。完了まで時間がかかる場合があります".to_string())
}

// --- Disk cleanup ---

#[tauri::command]
fn clean_temp_files(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    let mut dirs = Vec::new();
    if let Ok(tmp) = std::env::temp_dir().canonicalize() {
        dirs.push(tmp);
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(std::path::PathBuf::from(local).join("Temp"));
    }
    dirs.push(std::path::PathBuf::from(r"C:\Windows\Temp"));
    dirs.push(std::path::PathBuf::from(r"C:\Windows\Prefetch"));

    let mut deleted = 0u64;
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = path.is_dir();
                let result = if is_dir {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if result.is_ok() {
                    deleted += 1;
                }
            }
        }
    }
    Ok(format!("一時ファイルを削除しました ({}項目)", deleted))
}

// --- Tweaks ---

#[tauri::command]
fn apply_tweak(app: tauri::AppHandle, id: String) -> Result<String, String> {
    ensure_premium(&app)?;
    // 適用前に現在の設定値をバックアップ（復元用）
    let _ = backup_tweak(&app, &id);
    match id.as_str() {
        "disable-windows-search" => {
            run_cmd("sc", &["stop", "WSearch"]);
            if run_cmd("sc", &["config", "WSearch", "start=", "disabled"]) {
                Ok("Windows Searchを無効化しました".to_string())
            } else {
                Err("サービス設定に失敗しました（管理者権限が必要です）".to_string())
            }
        }
        "disable-superfetch" => {
            run_cmd("sc", &["stop", "SysMain"]);
            if run_cmd("sc", &["config", "SysMain", "start=", "disabled"]) {
                Ok("Superfetch (SysMain) を無効化しました".to_string())
            } else {
                Err("サービス設定に失敗しました（管理者権限が必要です）".to_string())
            }
        }
        "disable-hibernation" => {
            if run_cmd("powercfg", &["/hibernate", "off"]) {
                Ok("休止状態を無効化しました".to_string())
            } else {
                Err("休止状態の無効化に失敗しました".to_string())
            }
        }
        "disable-error-reporting" => {
            run_cmd("sc", &["stop", "WerSvc"]);
            run_cmd("sc", &["config", "WerSvc", "start=", "disabled"]);
            reg_add(r"HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting", "Disabled", "1", "REG_DWORD");
            Ok("Windowsエラー報告を無効化しました".to_string())
        }
        "disable-telemetry" => {
            reg_add(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", "0", "REG_DWORD");
            run_cmd("sc", &["stop", "DiagTrack"]);
            run_cmd("sc", &["config", "DiagTrack", "start=", "disabled"]);
            Ok("テレメトリを無効化しました".to_string())
        }
        "disable-game-bar" => {
            reg_add(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled", "0", "REG_DWORD");
            reg_add(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\GameDVR", "AllowGameDVR", "0", "REG_DWORD");
            Ok("Xbox Game Barを無効化しました".to_string())
        }
        "remove-cortana" => {
            let script = "Get-AppxPackage -AllUsers *Microsoft.549981C3F5F10* | Remove-AppxPackage -AllUsers -ErrorAction SilentlyContinue; 'done'";
            ps_output(script)?;
            Ok("Cortanaの削除を試行しました".to_string())
        }
        "remove-windows-tips" => {
            reg_add(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338388Enabled", "0", "REG_DWORD");
            reg_add(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SoftLandingEnabled", "0", "REG_DWORD");
            Ok("Windows Tips/通知を無効化しました".to_string())
        }
        _ => Err(format!("不明なtweak: {}", id)),
    }
}

// --- Game Profiles ---

#[tauri::command]
fn apply_game_profile(app: tauri::AppHandle, profile: String) -> Result<String, String> {
    ensure_premium(&app)?;
    match profile.as_str() {
        "fps" | "competitive" => {
            optimize_network(app.clone())?;
            reg_add(r"HKCU\Software\Microsoft\GameBar", "AutoGameModeEnabled", "1", "REG_DWORD");
            reg_add(r"HKCU\Software\Microsoft\GameBar", "AllowAutoGameMode", "1", "REG_DWORD");
            if profile == "competitive" {
                // 高パフォーマンス電源プラン
                run_cmd("powercfg", &["/setactive", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"]);
            }
            Ok(format!("{}プロファイルを適用しました", profile)).map(|_| {
                if profile == "fps" {
                    "FPSプロファイルを適用しました（ネットワーク最適化 + Game Mode）".to_string()
                } else {
                    "Competitiveプロファイルを適用しました（ネットワーク最適化 + Game Mode + 高パフォーマンス電源）".to_string()
                }
            })
        }
        "streaming" => {
            optimize_network(app.clone())?;
            // バランス電源プランに戻す
            run_cmd("powercfg", &["/setactive", "381b4222-f694-41f0-9685-ff5bb260df2e"]);
            Ok("Streamingプロファイルを適用しました（ネットワーク最適化 + バランス電源）".to_string())
        }
        _ => Err(format!("不明なプロファイル: {}", profile)),
    }
}

// --- Startup Apps ---

#[derive(Serialize)]
struct StartupApp {
    name: String,
    command: String,
    hive: String, // "HKCU" or "HKLM"
}

fn query_run_key(hive: &str) -> Vec<StartupApp> {
    let key = format!(r"{}\Software\Microsoft\Windows\CurrentVersion\Run", hive);
    let output = hidden_console(std::process::Command::new("reg"))
        .args(["query", &key])
        .output();
    let mut apps = Vec::new();
    if let Ok(o) = output {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("HKEY") || line.contains("REG_SZ") == false {
                // name and value are separated by spaces before REG_SZ
            }
            if let Some(idx) = line.find("    REG_") {
                let name = line[..idx].trim().to_string();
                let command = line[idx + 8..].trim().to_string();
                if !name.is_empty() && !command.is_empty() {
                    apps.push(StartupApp { name, command, hive: hive.to_string() });
                }
            }
        }
    }
    apps
}

#[tauri::command]
fn get_startup_apps() -> Result<Vec<StartupApp>, String> {
    let mut apps = query_run_key("HKCU");
    apps.extend(query_run_key("HKLM"));
    Ok(apps)
}

#[tauri::command]
fn set_startup_app(app: tauri::AppHandle, hive: String, name: String, command: String, enabled: bool) -> Result<String, String> {
    ensure_premium(&app)?;
    let key = format!(r"{}\Software\Microsoft\Windows\CurrentVersion\Run", hive);
    let backup_key = format!("{}|{}", hive, name);

    if enabled {
        // settings.json のバックアップから復元
        let mut settings = settings_get(app.clone())?;
        if let Some(cmd) = settings["startup_apps_backup"][&backup_key].as_str() {
            let cmd = cmd.to_string();
            if reg_add(&key, &name, &cmd, "REG_SZ") {
                if let Some(obj) = settings["startup_apps_backup"].as_object_mut() {
                    obj.remove(&backup_key);
                    let _ = settings_set(app.clone(), settings);
                }
                return Ok(format!("{} を有効化しました", name));
            }
        }
        // バックアップがない場合は引数のcommandで復元
        if reg_add(&key, &name, &command, "REG_SZ") {
            return Ok(format!("{} を有効化しました", name));
        }
        Err(format!("{} の有効化に失敗しました", name))
    } else {
        // 無効化: コマンドをバックアップしてから削除
        let mut settings = settings_get(app.clone())?;
        if settings["startup_apps_backup"].is_null() {
            settings["startup_apps_backup"] = json!({});
        }
        settings["startup_apps_backup"][&backup_key] = json!(command);
        let _ = settings_set(app.clone(), settings);
        if run_cmd("reg", &["delete", &key, "/v", &name, "/f"]) {
            Ok(format!("{} を無効化しました", name))
        } else {
            Err(format!("{} の無効化に失敗しました（管理者権限が必要な場合があります）", name))
        }
    }
}

// ===================== システム情報 / 監視 =====================

#[derive(Serialize)]
struct DiskInfo {
    name: String,
    mount: String,
    total: u64,
    available: u64,
}

#[derive(Serialize)]
struct SystemInfo {
    os_name: String,
    os_version: String,
    cpu_name: String,
    cpu_cores: usize,
    cpu_threads: usize,
    cpu_usage: f32,
    total_ram: u64,
    used_ram: u64,
    uptime_secs: u64,
    disks: Vec<DiskInfo>,
}

#[tauri::command]
fn get_system_info() -> Result<SystemInfo, String> {
    let mut sys = System::new_all();
    sys.refresh_all();
    // CPU使用率を取得するために少し待ってから再取得
    std::thread::sleep(std::time::Duration::from_millis(300));
    sys.refresh_all();

    let cpu_name = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
    let cpu_threads = sys.cpus().len();
    let cpu_cores = sys.physical_core_count().unwrap_or(cpu_threads);

    let disks_list = sysinfo::Disks::new_with_refreshed_list();
    let disks: Vec<DiskInfo> = disks_list.iter().map(|d| DiskInfo {
        name: d.name().to_string_lossy().to_string(),
        mount: d.mount_point().to_string_lossy().to_string(),
        total: d.total_space(),
        available: d.available_space(),
    }).collect();

    let total_ram = sys.total_memory();
    let used_ram = total_ram.saturating_sub(sys.available_memory());

    Ok(SystemInfo {
        os_name: System::name().unwrap_or_default(),
        os_version: System::kernel_version().unwrap_or_default(),
        cpu_name,
        cpu_cores,
        cpu_threads,
        cpu_usage: sys.global_cpu_usage(),
        total_ram,
        used_ram,
        uptime_secs: System::uptime(),
        disks,
    })
}

#[derive(Serialize)]
struct ProcStat {
    pid: u32,
    name: String,
    memory: u64,
    cpu: f32,
}

#[tauri::command]
fn get_process_list() -> Result<Vec<ProcStat>, String> {
    let mut sys = System::new_all();
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(250));
    sys.refresh_all();

    let mut procs: Vec<ProcStat> = sys.processes().iter().map(|(pid, p)| ProcStat {
        pid: pid.as_u32(),
        name: p.name().to_string_lossy().to_string(),
        memory: p.memory(),
        cpu: p.cpu_usage(),
    }).collect();
    procs.sort_by(|a, b| b.memory.cmp(&a.memory));
    procs.truncate(50);
    Ok(procs)
}

// --- ゲーム検出 ---

#[derive(Serialize)]
struct DetectedGame {
    pid: u32,
    name: String,
    title: String,
}

const GAME_KEYWORDS: &[&str] = &[
    "valorant", "cs2", "counter-strike", "counter strike", "csgo",
    "fortnite", "minecraft", "apex legends", "apex_legends", "overwatch",
    "dota", "league of legends", "warzone", "call of duty", "cod.exe",
    "pubg", "playerunknown", "genshin", "lost ark", "rust.exe", "ark.exe",
    "dayz", "tarkov", "escape from", "palworld", "helldivers", "destiny",
    "warframe", "final fantasy", "ffxiv", "ff14", "monster hunter",
    "elden ring", "cyberpunk", "witcher", "starfield", "baldur", "divinity",
    "bg3", "persona", "yakuza", "like a dragon", "nioh", "sekiro",
    "dark souls", "bloodborne", "hollow knight", "celeste", "hades",
    "dead cells", "terraria", "rimworld", "factorio", "stellaris",
    "crusader kings", "ck3", "total war", "civilization", "civ6", "xcom",
    "fire emblem", "zelda", "pokemon", "mario", "sonic", "kirby", "metroid",
    "halo", "gear", "forza", "fifa", "ea fc", "madden", "nba 2k",
    "wwe 2k", "ufc", "tekken", "street fighter", "mortal kombat", "mk11",
    "dragon ball", "one piece", "jujutsu", "gundam", "ace combat",
    "dcs", "il-2", "war thunder", "world of tanks", "worldoftanks",
    "tanki", "crossout", "robocraft", "sea of thieves", "raft",
    "valheim", "subnautica", "no man", "space engineers", "empyrion",
    "kingdom come", "greedfall", "outward", "scum", "miscreated",
    "7 days to die", "7dtd", "killing floor", "payday", "left 4 dead",
    "vermintide", "darktide", "space marine", "battlefield", "battlefield",
    "rush", "need for speed", "nfs", "grid", "dirt", "wrc", "rally",
    "assetto corsa", "acc.exe", "project cars", "iracing", "iracing",
    "rocket league", "fall guys", "among us", "phasmophobia", "content warning",
    "schedule i", "schedule1", "house flipper", "powerwash", "vrchat",
    "beat saber", "half-life", "hl2", "portal", "team fortress", "tf2",
    "left 4 dead 2", "l4d2", "insurgency", "squad", "hell let loose",
    "tarkov", "arena breakout", "once human", "night crows",
];

fn is_game_process(name: &str, title: &str) -> bool {
    let n = name.to_lowercase();
    let t = title.to_lowercase();
    // Zero自身や一般的なランチャー/ブラウザを除外
    if n.contains("zero") || n.contains("farst") || n.contains("code") {
        return false;
    }
    GAME_KEYWORDS.iter().any(|kw| n.contains(kw) || t.contains(kw))
}

#[tauri::command]
fn detect_games() -> Result<Vec<DetectedGame>, String> {
    let script = "Get-Process | Where-Object { $_.MainWindowTitle -ne '' } | ForEach-Object { \"$($_.Id)`t$($_.ProcessName)`t$($_.MainWindowTitle)\" }";
    let out = ps_output(script)?;
    let games: Vec<DetectedGame> = out.lines().filter_map(|line| {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 { return None; }
        let pid = parts[0].trim().parse::<u32>().ok()?;
        let name = parts[1].trim().to_string();
        let title = parts[2..].join("\t").trim().to_string();
        if is_game_process(&name, &title) {
            Some(DetectedGame { pid, name, title })
        } else {
            None
        }
    }).collect();
    Ok(games)
}

// --- ディスククリーン強化 ---

#[tauri::command]
fn clean_disk_deep(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    // まず通常の一時ファイル削除を実行
    let mut total = String::new();
    if let Ok(msg) = clean_temp_files(app.clone()) {
        total.push_str(&msg);
        total.push_str(" / ");
    }

    let mut deleted = 0u64;
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let local = std::path::PathBuf::from(local);
        dirs.push(local.join(r"CrashDumps"));
        dirs.push(local.join(r"Microsoft\Windows\INetCache"));
        dirs.push(local.join(r"Microsoft\Windows\Explorer")); // サムネイル/アイコンキャッシュ
        dirs.push(local.join(r"Microsoft\Edge\User Data\Default\Cache"));
        dirs.push(local.join(r"Google\Chrome\User Data\Default\Cache"));
    }
    dirs.push(std::path::PathBuf::from(r"C:\Windows\SoftwareDistribution\Download"));
    dirs.push(std::path::PathBuf::from(r"C:\Windows\Minidump"));
    dirs.push(std::path::PathBuf::from(r"C:\Windows\LiveKernelReports"));
    dirs.push(std::path::PathBuf::from(
        r"C:\Windows\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"
    ));

    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if result.is_ok() {
                    deleted += 1;
                }
            }
        }
    }

    // ごみ箱を空にする
    let _ = ps_output("Clear-RecycleBin -Force -ErrorAction SilentlyContinue; 'done'");

    total.push_str(&format!("詳細クリーン: {}項目を追加削除（Windows Update/ログ/キャッシュ/ごみ箱）", deleted));
    Ok(total)
}

// --- レジストリクリーナー（MRU/プライバシー系・安全な範囲のみ） ---

const REG_CLEAN_TARGETS: &[&str] = &[
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\RecentDocs",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\RunMRU",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\TypedPaths",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\WordWheelQuery",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\ComDlg32\LastVisitedPidlMRU",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\ComDlg32\LastVisitedMRU",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Search\RecentApps",
];

#[tauri::command]
fn registry_clean(app: tauri::AppHandle) -> Result<String, String> {
    ensure_premium(&app)?;
    let mut cleaned = 0u32;
    for key in REG_CLEAN_TARGETS {
        if run_cmd("reg", &["delete", key, "/f"]) {
            cleaned += 1;
        }
    }

    // アクティビティ履歴（タイムライン）をポリシーで無効化
    let mm = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\System";
    if reg_add(mm, "EnableActivityFeed", "0", "REG_DWORD") { cleaned += 1; }
    if reg_add(mm, "PublishUserActivities", "0", "REG_DWORD") { cleaned += 1; }
    if reg_add(mm, "UploadUserActivities", "0", "REG_DWORD") { cleaned += 1; }

    Ok(format!(
        "レジストリ/MRUデータをクリアしました ({}項目)。最近使ったファイル履歴やアクティビティ履歴が削除されます。",
        cleaned
    ))
}

// --- Tweak バックアップ / 復元 ---

fn reg_query_value(key: &str, name: &str) -> Option<(String, String)> {
    let out = hidden_console(std::process::Command::new("reg"))
        .args(["query", key, "/v", name])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let t = line.trim_start();
        if !t.starts_with(name) {
            continue;
        }
        // 値名の直後が空白であることを確認（部分一致を防ぐ）
        let rest_name = &t[name.len()..];
        if !rest_name.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
            continue;
        }
        if let Some(pos) = t.find("REG_") {
            let rest = &t[pos..];
            if let Some(ws) = rest.find(|c: char| c.is_whitespace()) {
                let kind = rest[..ws].to_string();
                let data = rest[ws..].trim().to_string();
                return Some((kind, data));
            }
        }
    }
    None
}

fn service_start_mode(name: &str) -> Option<String> {
    let safe = name.replace('\'', "''");
    let script = format!(
        "(Get-CimInstance Win32_Service -Filter \"Name='{}'\" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty StartMode)",
        safe
    );
    ps_output(&script).ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn tweak_reg_targets(id: &str) -> Vec<(&'static str, &'static str)> {
    match id {
        "disable-error-reporting" => vec![
            (r"HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting", "Disabled"),
        ],
        "disable-telemetry" => vec![
            (r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry"),
        ],
        "disable-game-bar" => vec![
            (r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled"),
            (r"HKLM\SOFTWARE\Policies\Microsoft\Windows\GameDVR", "AllowGameDVR"),
        ],
        "remove-windows-tips" => vec![
            (r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338388Enabled"),
            (r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SoftLandingEnabled"),
        ],
        _ => vec![],
    }
}

fn tweak_service_targets(id: &str) -> Vec<&'static str> {
    match id {
        "disable-windows-search" => vec!["WSearch"],
        "disable-superfetch" => vec!["SysMain"],
        "disable-error-reporting" => vec!["WerSvc"],
        "disable-telemetry" => vec!["DiagTrack"],
        _ => vec![],
    }
}

fn backup_tweak(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    if id == "remove-cortana" {
        return Ok(()); // 復元不可のためバックアップしない
    }
    let mut settings = settings_get(app.clone())?;
    if settings["tweak_backups"].is_null() {
        settings["tweak_backups"] = json!({});
    }
    // 既にバックアップがあれば上書きしない（適用済み状態を保存してしまうため）
    if settings["tweak_backups"][id].is_object() {
        return Ok(());
    }

    let mut reg = Vec::new();
    for (key, name) in tweak_reg_targets(id) {
        let q = reg_query_value(key, name);
        reg.push(json!({
            "key": key,
            "name": name,
            "exists": q.is_some(),
            "kind": q.as_ref().map(|x| x.0.clone()),
            "value": q.map(|x| x.1),
        }));
    }
    let mut svcs = Vec::new();
    for name in tweak_service_targets(id) {
        svcs.push(json!({
            "name": name,
            "startmode": service_start_mode(name),
        }));
    }
    let special = if id == "disable-hibernation" { json!(["hibernate_on"]) } else { json!([]) };

    settings["tweak_backups"][id] = json!({ "reg": reg, "services": svcs, "special": special });
    settings_set(app.clone(), settings)
}

#[tauri::command]
fn restore_tweak(app: tauri::AppHandle, id: String) -> Result<String, String> {
    ensure_premium(&app)?;
    if id == "remove-cortana" {
        return Err("Cortanaの削除は復元できません".to_string());
    }
    let mut settings = settings_get(app.clone())?;
    let backup = settings["tweak_backups"][&id].clone();
    if backup.is_null() {
        return Err("このtweakのバックアップがありません".to_string());
    }

    let mut restored = 0u32;
    if let Some(regs) = backup["reg"].as_array() {
        for r in regs {
            let key = r["key"].as_str().unwrap_or("");
            let name = r["name"].as_str().unwrap_or("");
            if r["exists"].as_bool().unwrap_or(false) {
                let kind = r["kind"].as_str().unwrap_or("REG_SZ");
                let value = r["value"].as_str().unwrap_or("");
                if reg_add(key, name, value, kind) { restored += 1; }
            } else if run_cmd("reg", &["delete", key, "/v", name, "/f"]) {
                restored += 1;
            }
        }
    }
    if let Some(svcs) = backup["services"].as_array() {
        for s in svcs {
            let name = s["name"].as_str().unwrap_or("");
            let mode = s["startmode"].as_str().unwrap_or("");
            let arg = match mode {
                "Auto" => "auto",
                "Manual" => "demand",
                "Disabled" => "disabled",
                _ => continue,
            };
            if run_cmd("sc", &["config", name, "start=", arg]) { restored += 1; }
        }
    }
    if let Some(special) = backup["special"].as_array() {
        if special.iter().any(|x| x.as_str() == Some("hibernate_on")) {
            if run_cmd("powercfg", &["/hibernate", "on"]) { restored += 1; }
        }
    }

    if let Some(obj) = settings["tweak_backups"].as_object_mut() {
        obj.remove(&id);
        let _ = settings_set(app.clone(), settings);
    }
    Ok(format!("{} を復元しました ({}項目)", id, restored))
}

fn is_essential_process(name: &str) -> bool {
    let essential_processes = vec![
        "System", "System Idle Process", "smss.exe", "csrss.exe",
        "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe",
        "svchost.exe", "explorer.exe", "dwm.exe", "conhost.exe",
        "rustup-init.exe", "cargo.exe", "FARST.exe", "app.exe", "TOIRE.exe", "Zero.exe", "zero.exe",
    ];

    essential_processes.contains(&name)
}

#[tauri::command]
fn win_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn win_close(window: tauri::Window) {
    let _ = window.close();
}

#[tauri::command]
fn win_toggle_maximize(window: tauri::Window) -> bool {
    let maximized = window.is_maximized().unwrap_or(false);
    if maximized {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
    !maximized
}

#[tauri::command]
fn win_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}
