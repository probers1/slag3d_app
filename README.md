# SLAG3D 🖨️ — Multi-User 3D Model Repository

A self-hosted mesh repository and dashboard for 3D printer models, print farms, and workshop teams.

---

## Features

| Feature | Description |
|---------|-------------|
| **Headless Thumbnail Engine** | Puppeteer + Three.js → 512×512 PNGs (concurrency: 2, exponential retry) |
| **3D Model Viewer** | Embedded Three.js viewer (.stl, .3mf, .obj with OrbitControls, wireframe toggle) |
| **Mesh Analytics** | Bounding box, volume (cm³), weight estimate, filament cost calculator |
| **Archive Extraction** | Multi-file uploads (.zip, .7z → auto-extract, detect primary 3D file) |
| **Collections & Folders** | Nested folder hierarchy with model organization |
| **Dual Authentication** | Session cookies + JWT Bearer tokens (iframe-safe) |
| **Admin Panel** | User management, provisioning, password resets |

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Runtime | Node.js 20 / Express 4 |
| Database | SQLite (`better-sqlite3`, WAL mode) |
| Frontend | React 19, TypeScript, Tailwind CSS |
| 3D Engine | Three.js with STLLoader, OBJLoader, ThreeMFLoader |
| **Desktop** | **Tauri v2 (Rust backend + native WebView)** |

---

## Quick Start

### Development (Web Server)
```bash
cp .env.example .env   # Fill in the three secrets, set GOOSE_SEED_DB=true
npm install && npm run dev
# First run seeds: admin / admin123 (change immediately)
```

### Development (Desktop App)
```bash
npm install && npm run tauri dev
# Launches Vite dev server + Tauri native window with hot reload
```

### Production (Web Server)
```bash
cp .env.example .env   # Fill in all secrets and a strong ADMIN_PASSWORD
npm run build && npm start
```

### Production (Desktop App)
```bash
npm run build:desktop
# Builds native installers for all platforms
```

---

## Environment Variables

```bash
JWT_SECRET=<min 32 chars>          # Required
REFRESH_TOKEN_SECRET=<min 32>      # Required
SESSION_SECRET=<min 32 chars>      # Required
GOOSE_SEED_DB=true                # Seed first admin (dev / first deploy)
ADMIN_USER=admin                  # Optional, defaults to admin
ADMIN_PASSWORD=<strong password>  # Seeds this password on first run
SESSION_EXPIRY_MINUTES=1440       # Default: 24 hours
```

Generate secret: `openssl rand -base64 52`

---

## File Uploads

**Allowed**: `.stl`, `.3mf`, `.obj`, `.zip`, `.7z` (max 300MB)
**Rate Limit**: 30 requests/hour per IP

Security checks block executables, scripts, image files.

---

## Database Schema

```sql
users, models, files, tags, model_tags, 
favorites, collections, collection_models, auth_tokens
```

Schema version **4** — see [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md)

---

## API Quick Reference

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| GET | `/api/models` | ✓ | Paginated search (search, tags, filters) |
| POST | `/api/models` | ✓ | Multi-file upload or archive extraction |
| POST | `/api/auth/login` | — | Returns access + refresh tokens |
| POST | `/api/auth/refresh` | ✓ | Token rotation endpoint |

See full API reference in docs folder.

---

## Desktop App (Tauri v2)

SLAG3D now includes a native cross-platform desktop application built with Tauri v2.

### Architecture
- **Frontend**: React 19 + Three.js + Tailwind CSS (runs in native OS WebView)
- **Backend**: Rust (SQLite via `rusqlite`, native zip/7z extraction, custom asset protocol)
- **Platforms**: Linux, Windows, macOS

### Prerequisites

#### Linux (Ubuntu/Debian)
```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-0 \
  libgtk-3-0 \
  libayatana-appindicator3-1 \
  librsvg2-common \
  build-essential \
  curl \
  wget \
  libssl-dev \
  libappindicator3-dev \
  librsvg2-dev \
  libwebkit2gtk-4.1-dev
```

#### Windows
- Rust toolchain: `rustup target add x86_64-pc-windows-gnu`
- Visual Studio Build Tools or MinGW-w64

#### macOS
- Xcode Command Line Tools: `xcode-select --install`

### Building Desktop Packages

The desktop app is built using `tauri build` which creates native installers for each platform.

#### Build Commands

| Command | Description |
|---------|-------------|
| `npm run tauri dev` | Development mode with hot reload |
| `npm run tauri build` | Production build for current platform |
| `npm run build:desktop` | Alias for `tauri build` |

#### Output Formats

| Platform | Formats |
|----------|---------|
| **Linux** | `.AppImage`, `.deb` |
| **Windows** | `.msi`, `.exe` (NSIS installer) |
| **macOS** | `.dmg`, `.app` |

#### Cross-Compilation Notes

To build for other platforms, you need the appropriate Rust targets:
```bash
# Windows from Linux
rustup target add x86_64-pc-windows-gnu

# Linux from Windows
rustup target add x86_64-unknown-linux-gnu
```

For macOS, you must build on macOS hardware.

#### Build Output Locations

After `npm run build:desktop`, packages are created in:
```
src-tauri/target/release/bundle/
├── appimage/         # Linux .AppImage
├── deb/              # Linux .deb
├── msi/              # Windows .msi
├── nsis/             # Windows .exe installer
└── dmg/              # macOS .dmg
```

### Desktop App Features
- **Zero authentication** — runs as a local single-user app
- **Native file dialogs** — OS file pickers for importing models
- **Local storage** — `~/.local/share/slag3d` (Linux), `%APPDATA%\slag3d` (Windows), `~/Library/Application Support/slag3d` (macOS)
- **WebGL thumbnails** — rendered client-side, no Puppeteer needed
- **Custom protocol** — `slag3d-asset://` for fast local file streaming

---

## Deployment

### Proxmox LXC (Recommended for Web Server)
```bash
curl -fsSL https://raw.githubusercontent.com/slag3d/slag3d/main/scripts/install-lxc.sh | bash
systemctl status slag3d.service
```

The installer generates a random admin password, stores it in `/opt/slag3D/.env`
and prints it at the end. It also verifies that the printed credentials really
work and re-syncs the admin account if the database contains an older password.

---

## Default Admin Credentials

| Username | Password | Role | Notes |
|----------|----------|------|-------|
| `admin` | `admin123` | Administrator | **Dev mode only** — change after first login |
| `admin` | random | Administrator | LXC installer — see `/opt/slag3D/.env` (`ADMIN_PASSWORD`) |

Change default password via: Settings → Change Password (top navigation)

### Resetting the admin password (server console)

```bash
cd /opt/slag3D
npm run set-admin -- 'YourNewStrongPassword'   # one-off password
npm run set-admin                               # uses ADMIN_USER / ADMIN_PASSWORD from .env
```

The command updates the `admin` account in the database directly, so it also
works when the current password is unknown or the account was soft-deleted.

---

## Security Guidelines

- Always set `JWT_SECRET` in production (min 32 characters)
- Never commit `.env` files to Git
- Rotate secrets every 90 days
- Use HTTPS/HSTS for public access
- See [SECURITY_GUIDE.md](./SECURITY_GUIDE.md) for full implementation

---

## Support

- Issues: Open GitHub issues for bugs/features
- Documentation: See related guides below

---

## Related Documentation

- [AGENTS.md](./AGENTS.md) — Developer quick reference
- [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md) — Database migrations
- [SECURITY_GUIDE.md](./SECURITY_GUIDE.md) — Security implementation
- [Tauri Conversion Design](./docs/superpowers/specs/2026-11-09-tauri-conversion-design.md) — Desktop app architecture

---

**Version**: 4 (Soft delete active)
**Last Updated**: Nov 2026