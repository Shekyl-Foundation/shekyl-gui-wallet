# Installing the Shekyl Wallet

Download the latest release from
[GitHub Releases](https://github.com/Shekyl-Foundation/shekyl-gui-wallet/releases).

---

## Linux

### AppImage (recommended -- works on any distro)

1. Download `Shekyl_Wallet_X.Y.Z_amd64.AppImage`.
2. Make it executable and run:

```bash
chmod +x "Shekyl_Wallet_X.Y.Z_amd64.AppImage"
./"Shekyl_Wallet_X.Y.Z_amd64.AppImage"
```

The AppImage bundles all required libraries. No installation step is needed.

To integrate it with your desktop launcher, use
[AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) or move
the file to `~/Applications/` and create a `.desktop` entry manually.

### Debian / Ubuntu (.deb)

```bash
sudo dpkg -i "Shekyl_Wallet_X.Y.Z_amd64.deb"
```

If dependency errors appear:

```bash
sudo apt --fix-broken install
```

The wallet will appear in your application menu as **Shekyl Wallet** and can
also be launched from the terminal:

```bash
shekyl-wallet
```

To uninstall:

```bash
sudo apt remove shekyl-wallet
```

### Fedora / RHEL / openSUSE (.rpm)

```bash
sudo rpm -i "Shekyl_Wallet-X.Y.Z-1.x86_64.rpm"
```

Or with `dnf`:

```bash
sudo dnf install "Shekyl_Wallet-X.Y.Z-1.x86_64.rpm"
```

To uninstall:

```bash
sudo dnf remove shekyl-wallet
```

---

## Windows

### Installer (.exe / NSIS)

1. Download `Shekyl_Wallet_X.Y.Z_x64-setup.exe`.
2. Run the installer and follow the prompts.
3. The wallet is added to the Start Menu.

### MSI

1. Download `Shekyl_Wallet_X.Y.Z_x64.msi`.
2. Double-click to install, or run from an elevated command prompt:

```powershell
msiexec /i "Shekyl_Wallet_X.Y.Z_x64.msi"
```

To uninstall, use **Settings > Apps** or:

```powershell
msiexec /x "Shekyl_Wallet_X.Y.Z_x64.msi"
```

### System requirements

- Windows 10 (version 1803+) or Windows 11.
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
  is required. Windows 11 includes it by default. On Windows 10 the installer
  will prompt you to download it if missing.

---

## macOS

### DMG

1. Download `Shekyl_Wallet_X.Y.Z_x64.dmg` (Intel) or
   `Shekyl_Wallet_X.Y.Z_aarch64.dmg` (Apple Silicon).
2. Open the `.dmg` and drag **Shekyl Wallet** into your **Applications** folder.
3. On first launch, macOS may warn that the app is from an unidentified
   developer. Go to **System Settings > Privacy & Security** and click
   **Open Anyway**, or run:

```bash
xattr -cr /Applications/Shekyl\ Wallet.app
```

### System requirements

- macOS 10.15 (Catalina) or later.
- Apple Silicon (M1/M2/M3) and Intel are both supported.

---

## First launch

When you open the Shekyl Wallet for the first time:

1. **Create a new wallet** or **open an existing wallet file**.
2. Configure the **daemon connection** under **Settings**:
   - MainNet default: `http://127.0.0.1:11029`
   - TestNet default: `http://127.0.0.1:12029`
   - StageNet default: `http://127.0.0.1:13029`
3. The wallet will sync with the daemon. Progress is shown in the header bar.

You need a running `shekyld` daemon for the wallet to function. See the
[shekyl-core documentation](https://github.com/Shekyl-Foundation/shekyl-core)
for daemon setup instructions.

---

## Verifying downloads

Release assets include SHA-256 checksums. After downloading, verify:

```bash
# Linux / macOS
sha256sum "Shekyl_Wallet_X.Y.Z_amd64.AppImage"
# Compare the output against the published checksum in the release notes
```

```powershell
# Windows (PowerShell)
Get-FileHash "Shekyl_Wallet_X.Y.Z_x64-setup.exe" -Algorithm SHA256
```

---

## Troubleshooting

### Linux: "WebKitGTK not found" or blank window

Install the WebKitGTK 4.1 runtime:

```bash
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-0

# Fedora
sudo dnf install webkit2gtk4.1

# Arch
sudo pacman -S webkit2gtk-4.1
```

### Windows: blank white window

Install or update the
[WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

### macOS: "Shekyl Wallet is damaged and can't be opened"

This happens when the Gatekeeper quarantine flag is set. Remove it:

```bash
xattr -cr /Applications/Shekyl\ Wallet.app
```

### Wallet won't sync

Ensure `shekyld` is running and listening on the configured RPC port.
Check the daemon status:

```bash
curl -s http://127.0.0.1:11029/json_rpc \
  -d '{"jsonrpc":"2.0","id":"0","method":"get_info"}' \
  -H 'Content-Type: application/json' | head -c 200
```

If no response, the daemon is not running or is bound to a different address.

---

## Updating

Download the new version from the
[Releases page](https://github.com/Shekyl-Foundation/shekyl-gui-wallet/releases)
and install it over the existing version. Wallet files and settings are stored
in your home directory and are preserved across updates.

| Platform | Wallet data location |
|----------|---------------------|
| Linux | `~/.local/share/shekyl-wallet/` |
| macOS | `~/Library/Application Support/shekyl-wallet/` |
| Windows | `%APPDATA%\shekyl-wallet\` |
