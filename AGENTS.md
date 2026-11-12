# Zero (旧TOIRE / FARST) - プロジェクト引き継ぎメモ

## 概要
Windows専用のゲーマー向け軽量化・最適化ツール。Tauri 2 + Rust + 静的HTML (dist/index.html) のデスクトップアプリ。
リポジトリ: https://github.com/wasaomen/FARST

## 構成
- `src-tauri/` — Tauriアプリ本体 (`zero` クレート、`zero_lib`)。コマンドは `src-tauri/src/lib.rs`
- `dist/index.html` — フロントエンド全て（単一HTML、inline CSS/JS、`window.__TAURI__.core.invoke` 使用）
- `src/main.rs` — CLI版（FARST.exe、同等機能のターミナルUI）
- `dist/logo.png` — アプリロゴ（src-tauri/icons/Zero.png のコピー）

## 現在のバージョン: 1.0.0（システムモニター・ゲーム自動検出・スケジューラ等の新機能追加）
- productName: **Zero**、identifier: `com.zero.app`（旧TOIRE `com.toire.app` から変更済み → 別アプリ扱い、旧TOIREは手動アンインストール必要）
- ウィンドウ: 1180x740（最小 1024x660）、UIはガラスモーフィズム + 角丸 + コンパクト

## 実装済み機能
- メモリ解放 (EmptyWorkingSet)、不要プロセス終了、サービス停止
- `optimize_network`: Nagle無効化(TCPNoDelay/TcpAckFrequency/TCPDelAckTicks)、NetworkThrottlingIndex=0xffffffff、SystemResponsiveness=0、Games タスク優先度、netsh (autotuning=high, ecn/timestamps無効, fastopen有効)、DNSフラッシュ
- `network_boost`（悪い回線向け）: CTCP輻輳プロバイダ、Game Mode強制、NonBestEffortLimit=0、DoSvc停止+無効化、NIC省電力(EEE/Green Ethernet/Power Saving/Interrupt Moderation)無効化（PowerShell経由）、DNSフラッシュ
- `ping_test`: 1.1.1.1へping 4回、平均ms返す
- 実用機能: 復元ポイント作成/一覧、DNSフラッシュ、ネットワーク修復(winsock/ip reset)、sfc /scannow・chkdsk /scan（別プロセス起動）、一時ファイル削除、ディープクリーン(clean_disk_deep: WinUpdate/ログ/ブラウザキャッシュ/ごみ箱)、レジストリクリーナー(registry_clean: MRU/RecentDocs等の安全な項目+アクティビティ履歴無効化)、tweak適用（WSearch/SysMain/Hibernation/WER/Telemetry/GameDVR/Cortana/WindowsTips、適用前にsettings.jsonへ自動バックアップ→Applied状態で再クリックするとrestore_tweakで復元）、ゲームプロファイル（fps/competitive/streaming）、起動アプリ一覧+有効/無効切替（Run レジストリ、無効化時はsettings.jsonにバックアップ）
- 監視系: get_system_info（CPU/RAM/ディスク/OS情報のみ。GPU情報は重いので廃止済み）、get_process_list（CPU%+メモリ上位50）、detect_games（ウィンドウタイトル持ちプロセスをGAME_KEYWORDSで判定）
- 自動化: スケジューラページ（settings.schedule={enabled,time,action,last_run}、30秒ごとにcheckScheduler→optimize/clean/boost実行、アプリ起動中のみ動作）、ゲーム自動検出（settings.game_auto_profile、15秒ごとにdetect_games→新規PIDにcompetitiveプロファイル自動適用）
- ダッシュボードは5秒ごとにloadSystemStatusで自動更新（CPU/GPUゲージ・ディスクバー・Resourcesページも実データ表示）
- UI: トースト通知(showToast)、シンプルなロード画面（ロゴ+プログレスバー）、ガラスモーフィズム+丸みUI
- GitHub連携（Accountページ）: OAuth Device Flowでログインし、設定をシークレットGist "Zero Settings"（settings.json）と同期
  - **設定済み**: `GITHUB_CLIENT_ID` = `Ov23lihk8weUr2EV6HXk`（src-tauri/src/lib.rs）。OAuth Appは https://github.com/settings/developers で管理
  - 認証情報は `%APPDATA%/com.zero.app/auth.json`、ローカル設定は同 `settings.json` に保存
  - 依存: `ureq`（HTTPクライアント、src-tauri/Cargo.toml）

## アップデート/リリースの仕組み（重要）
- `tauri.conf.json` の updater endpoint: `https://github.com/wasaomen/FARST/releases/latest/download/latest.json`
- 署名鍵: `C:\Users\harum\.tauri\FARST-updater.key`（秘密鍵・**絶対にコミットしない**）、公開鍵は tauri.conf.json の `plugins.updater.pubkey` に設定済み
- **旧鍵の問題**: v0.3.5 以前は別の公開鍵(AB425F01FE43B56C)でビルドされており、その秘密鍵は消失。0.3.5→新バージョンの自動アップデートは不可（署名検証で "Invalid encoding in minisign data" エラー）。Zeroユーザーは手動インストールが必要
- `.github/workflows/release.yml`: `v*` タグpushで Windows ビルド+署名+ドラフトリリース作成（tauri-action）。要 GitHub Secrets: `TAURI_SIGNING_PRIVATE_KEY`（鍵ファイルの中身）, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（空でOK）

## リリース手順（毎回これ）

### エージェント側の作業
1. バージョンを3ファイルで更新: ルート `Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`
2. `cd src-tauri && cargo tauri build`（キャッシュ時は.sigが自動生成されないことがある）
3. `.sig`が無ければ手動署名:
   `cargo tauri signer sign "target/release/bundle/nsis/Zero_X.Y.Z_x64-setup.exe" --private-key-path "C:\Users\harum\.tauri\FARST-updater.key" --app-version X.Y.Z`
4. `src-tauri/latest.json` の version / notes / pub_date / signature（署名出力のPublic signature）/ url を更新
   - url は `releases/download/vX.Y.Z/Zero_X.Y.Z_x64-setup.exe` 形式
5. ユーザーに以下の形式でGitHub手動作業を依頼する:

### GitHubでの作業（手動）
```
https://github.com/wasaomen/FARST/releases → Draft a new release
Choose a tag: `vX.Y.Z`
以下の3ファイルをアップロード:
  - `Zero_X.Y.Z_x64-setup.exe`
  - `Zero_X.Y.Z_x64-setup.exe.sig`
  - `latest.json`
Release notes例: `<変更内容の要約>`
Publish release（必須）
これで旧バージョンのユーザーはアプリ内から自動アップデートできます。
毎回この返信をしてください:
```

6. 既存リリースの差し替え時は Edit release → 旧アセット削除 → 新アセット追加

## ビルド/検証コマンド
- `cargo build`（ルート: CLI版）/ `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo tauri build`（src-tauri内、リリースバンドル）

## 既知の警告（無害）
- identifier が `.app` で終わる警告（macOS向け、Windowsのみなら無視）
- `dist/Untitled.png` の NSIS 5040 警告（ヘッダー画像フォーマット非対応）

## ユーザーの好み・傾向
- 日本語でのやり取り。UIは「見やすく」「透明感（ガラスモーフィズム）」「丸み」「コンパクト」を好む
- 毎回バージョンを上げてリリースする流れ（0.4.5→0.5.0→0.5.5→0.6.0）
- リリース作業は基本的に私（エージェント）がビルド・署名・latest.json更新まで行い、GitHubへのアップロードはユーザーが手動で行う
