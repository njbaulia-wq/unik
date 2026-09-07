#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# FluxCut — Native Linux Video Editor Universal Installer
# Repository: https://github.com/njbaulia-wq/unik
# ==============================================================================

BOLD='\033[1m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

REPO="njbaulia-wq/unik"
BIN_NAME="fluxcut"
APP_ID="org.fluxcut.FluxCut"

INSTALL_DIR="${HOME}/.local/bin"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"
METAINFO_DIR="${HOME}/.local/share/metainfo"

print_banner() {
    echo -e "${BLUE}${BOLD}"
    echo "  ███████╗██╗     ██╗   ██╗██╗  ██╗ ██████╗██╗   ██╗████████╗"
    echo "  ██╔════╝██║     ██║   ██║╚██╗██╔╝██╔════╝██║   ██║╚══██╔══╝"
    echo "  █████╗  ██║     ██║   ██║ ╚███╔╝ ██║     ██║   ██║   ██║   "
    echo "  ██╔══╝  ██║     ██║   ██║ ██╔██╗ ██║     ██║   ██║   ██║   "
    echo "  ██║     ███████╗╚██████╔╝██╔╝ ██╗╚██████╗╚██████╔╝   ██║   "
    echo "  ╚═╝     ╚══════╝ ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═════╝    ╚═╝   "
    echo -e "${NC}"
    echo -e "  Native, Fast Linux Desktop Video Editor (GTK4 • Wayland • Rust • FFmpeg)"
    echo "-------------------------------------------------------------------------"
}

check_platform() {
    OS="$(uname -s)"
    if [ "${OS}" != "Linux" ]; then
        echo -e "${RED}[ERROR] FluxCut is exclusively designed for Linux desktop environments (Wayland/X11).${NC}"
        exit 1
    fi

    ARCH="$(uname -m)"
    case "${ARCH}" in
        x86_64) ARCH_NAME="x86_64" ;;
        aarch64|arm64) ARCH_NAME="aarch64" ;;
        *)
            echo -e "${YELLOW}[WARNING] Architecture ${ARCH} may not have pre-built binaries.${NC}"
            ARCH_NAME="${ARCH}"
            ;;
    esac
}

install_from_release() {
    echo -e "${BLUE}==>${NC} Checking for latest release from GitHub (${REPO})…"
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "${TMP_DIR}"' EXIT

    RELEASE_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)"
    ASSET_URL=""

    # Check for distro-specific asset (e.g. Fedora)
    DISTRO=""
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        DISTRO="${ID:-}"
    fi

    if [ -n "${RELEASE_JSON}" ]; then
        if [ "${DISTRO}" = "fedora" ]; then
            ASSET_URL="$(echo "${RELEASE_JSON}" | grep "browser_download_url.*fluxcut-fedora.*${ARCH_NAME}.*tar.gz" | head -n 1 | cut -d '"' -f 4 || true)"
        fi
        if [ -z "${ASSET_URL}" ]; then
            ASSET_URL="$(echo "${RELEASE_JSON}" | grep "browser_download_url.*fluxcut-.*${ARCH_NAME}.*tar.gz" | head -n 1 | cut -d '"' -f 4 || true)"
        fi
    fi

    if [ -n "${ASSET_URL}" ]; then
        echo -e "${BLUE}==>${NC} Downloading release binary: ${ASSET_URL}"
        curl -fSL "${ASSET_URL}" -o "${TMP_DIR}/fluxcut.tar.gz"
        tar -xzf "${TMP_DIR}/fluxcut.tar.gz" -C "${TMP_DIR}"
        install -Dm755 "${TMP_DIR}/fluxcut" "${INSTALL_DIR}/${BIN_NAME}"
    else
        # Fallback: if installer is executed inside repo or source directory
        SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        if [ -f "${SCRIPT_DIR}/target/release/${BIN_NAME}" ]; then
            echo -e "${BLUE}==>${NC} Found compiled release binary in repository."
            install -Dm755 "${SCRIPT_DIR}/target/release/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
        elif [ -f "${SCRIPT_DIR}/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
            echo -e "${YELLOW}==>${NC} No pre-built release asset found. Building from source via cargo…"
            (cd "${SCRIPT_DIR}" && cargo build --release --workspace)
            install -Dm755 "${SCRIPT_DIR}/target/release/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
        else
            echo -e "${RED}[ERROR] Could not find pre-built binary and cargo is not installed.${NC}"
            echo -e "Please install Rust/Cargo (https://rustup.rs) or download a binary release."
            exit 1
        fi
    fi
}

install_desktop_integration() {
    echo -e "${BLUE}==>${NC} Installing desktop integration files…"
    mkdir -p "${DESKTOP_DIR}" "${ICON_DIR}" "${METAINFO_DIR}"

    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    if [ -d "${SCRIPT_DIR}/resources" ]; then
        cp -f "${SCRIPT_DIR}/resources/${APP_ID}.desktop" "${DESKTOP_DIR}/" || true
        cp -f "${SCRIPT_DIR}/resources/${APP_ID}.metainfo.xml" "${METAINFO_DIR}/" || true
        cp -f "${SCRIPT_DIR}/resources/icons/hicolor/scalable/apps/${APP_ID}.svg" "${ICON_DIR}/" || true
    else
        # Download resource assets from GitHub main branch
        BASE_RAW="https://raw.githubusercontent.com/${REPO}/main/resources"
        curl -fsSL "${BASE_RAW}/${APP_ID}.desktop" -o "${DESKTOP_DIR}/${APP_ID}.desktop" || true
        curl -fsSL "${BASE_RAW}/${APP_ID}.metainfo.xml" -o "${METAINFO_DIR}/${APP_ID}.metainfo.xml" || true
        curl -fsSL "${BASE_RAW}/icons/hicolor/scalable/apps/${APP_ID}.svg" -o "${ICON_DIR}/${APP_ID}.svg" || true
    fi

    # Update desktop database if available
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${DESKTOP_DIR}" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
    fi
}

verify_installation() {
    echo ""
    if [ -x "${INSTALL_DIR}/${BIN_NAME}" ]; then
        echo -e "${GREEN}${BOLD}✓ FluxCut binary installed to: ${INSTALL_DIR}/${BIN_NAME}${NC}"
        
        # Check PATH
        if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
            echo -e "${YELLOW}[NOTE] ${INSTALL_DIR} is not currently in your \$PATH.${NC}"
            echo -e "Add it by adding this line to your ~/.bashrc or ~/.zshrc:"
            echo -e "  export PATH=\"\${HOME}/.local/bin:\$PATH\""
        fi

        # Check for missing shared libraries
        MISSING_LIBS="$(ldd "${INSTALL_DIR}/${BIN_NAME}" 2>/dev/null | grep "not found" || true)"
        if [ -n "${MISSING_LIBS}" ]; then
            echo ""
            echo -e "${YELLOW}${BOLD}[NOTICE] Missing system shared libraries detected in pre-built binary:${NC}"
            echo "${MISSING_LIBS}" | sed 's/^/  /'
            echo ""
            echo -e "${BLUE}${BOLD}==> Auto-repairing: Building FluxCut natively with your system's FFmpeg and GTK4...${NC}"
            install_from_source

            # Re-verify after native build
            NEW_MISSING="$(ldd "${INSTALL_DIR}/${BIN_NAME}" 2>/dev/null | grep "not found" || true)"
            if [ -n "${NEW_MISSING}" ]; then
                echo -e "${RED}[ERROR] Libraries still unresolved after native build:${NC}"
                echo "${NEW_MISSING}" | sed 's/^/  /'
                exit 1
            fi
        fi

        echo ""
        echo -e "${GREEN}${BOLD}✓ FluxCut is ready to use!${NC}"
        echo -e "Run diagnostics to verify hardware acceleration:"
        echo -e "  ${BOLD}fluxcut --diagnostics${NC}"
        echo ""
        echo -e "Launch FluxCut:"
        echo -e "  ${BOLD}fluxcut${NC}"
    else
        echo -e "${RED}[ERROR] Installation verification failed.${NC}"
        exit 1
    fi
}

install_from_source() {
    echo -e "${BLUE}==>${NC} Preparing native build environment..."

    DISTRO=""
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        DISTRO="${ID:-}"
    fi

    # Check and install system packages if needed
    if [ "${DISTRO}" = "fedora" ]; then
        if ! pkg-config --exists libavutil 2>/dev/null || ! pkg-config --exists gtk4 2>/dev/null; then
            echo -e "${BLUE}==>${NC} Installing Fedora development libraries (GTK4, Libadwaita, FFmpeg, Clang)..."
            if command -v sudo >/dev/null 2>&1; then
                sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel ffmpeg-free-devel clang
            elif [ "$(id -u)" -eq 0 ]; then
                dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel ffmpeg-free-devel clang
            else
                echo -e "${YELLOW}[WARNING] Sudo not available to install packages. If build fails, run:${NC}"
                echo -e "  sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel ffmpeg-free-devel clang"
            fi
        fi
    elif [ "${DISTRO}" = "ubuntu" ] || [ "${DISTRO}" = "debian" ]; then
        if ! pkg-config --exists libavutil 2>/dev/null || ! pkg-config --exists gtk4 2>/dev/null; then
            echo -e "${BLUE}==>${NC} Installing Ubuntu development libraries..."
            if command -v sudo >/dev/null 2>&1; then
                sudo apt-get update && sudo apt-get install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavfilter-dev clang
            fi
        fi
    elif [ "${DISTRO}" = "arch" ] || [ "${DISTRO}" = "manjaro" ]; then
        if ! pkg-config --exists libavutil 2>/dev/null || ! pkg-config --exists gtk4 2>/dev/null; then
            echo -e "${BLUE}==>${NC} Installing Arch development libraries..."
            if command -v sudo >/dev/null 2>&1; then
                sudo pacman -S --needed --noconfirm base-devel pkgconf gtk4 libadwaita ffmpeg clang
            fi
        fi
    fi

    # Check if cargo is installed
    if ! command -v cargo >/dev/null 2>&1; then
        echo -e "${YELLOW}[INFO] Rust toolchain not found. Installing Rust via rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        source "${HOME}/.cargo/env" 2>/dev/null || true
        export PATH="${HOME}/.cargo/bin:${PATH}"
    fi

    echo -e "${BLUE}==>${NC} Compiling fluxcut-app natively via cargo (links directly to system libraries)..."
    cargo install --git "https://github.com/${REPO}.git" fluxcut-app --root "${HOME}/.local" --force
    echo -e "${GREEN}${BOLD}✓ Native compilation and installation completed!${NC}"
}

main() {
    print_banner
    check_platform

    BUILD_FROM_SOURCE=false
    for arg in "$@"; do
        case "${arg}" in
            --build|--source|-b)
                BUILD_FROM_SOURCE=true
                ;;
            --help|-h)
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --build, --source, -b    Build natively from source via cargo (recommended for Fedora 40+, Arch, etc.)"
                echo "  --help, -h               Show this help message"
                exit 0
                ;;
        esac
    done

    if [ "${BUILD_FROM_SOURCE}" = true ]; then
        install_from_source
    else
        install_from_release
    fi

    install_desktop_integration
    verify_installation
}

main "$@"
