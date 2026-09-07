#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# FluxCut — Uninstaller Script
# Repository: https://github.com/njbaulia-wq/unik
# ==============================================================================

BOLD='\033[1m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BIN_PATH="${HOME}/.local/bin/fluxcut"
CARGO_BIN_PATH="${HOME}/.cargo/bin/fluxcut"
DESKTOP_FILE="${HOME}/.local/share/applications/org.fluxcut.FluxCut.desktop"
ICON_FILE="${HOME}/.local/share/icons/hicolor/scalable/apps/org.fluxcut.FluxCut.svg"
METAINFO_FILE="${HOME}/.local/share/metainfo/org.fluxcut.FluxCut.metainfo.xml"
CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/fluxcut"
CACHE_DIR="${XDG_CACHE_HOME:-${HOME}/.cache}/fluxcut"
DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/fluxcut"

echo -e "${BLUE}${BOLD}==> Uninstalling FluxCut...${NC}"

# Remove binary executables
FOUND_ANY=false
if [ -f "${BIN_PATH}" ]; then
    rm -f "${BIN_PATH}"
    echo -e "${GREEN}✓ Removed binary: ${BIN_PATH}${NC}"
    FOUND_ANY=true
fi
if [ -f "${CARGO_BIN_PATH}" ]; then
    rm -f "${CARGO_BIN_PATH}"
    echo -e "${GREEN}✓ Removed cargo binary: ${CARGO_BIN_PATH}${NC}"
    FOUND_ANY=true
fi

# Remove desktop integration
if [ -f "${DESKTOP_FILE}" ]; then
    rm -f "${DESKTOP_FILE}"
    echo -e "${GREEN}✓ Removed desktop launcher: ${DESKTOP_FILE}${NC}"
    FOUND_ANY=true
fi
if [ -f "${ICON_FILE}" ]; then
    rm -f "${ICON_FILE}"
    echo -e "${GREEN}✓ Removed desktop icon: ${ICON_FILE}${NC}"
    FOUND_ANY=true
fi
if [ -f "${METAINFO_FILE}" ]; then
    rm -f "${METAINFO_FILE}"
    echo -e "${GREEN}✓ Removed metainfo: ${METAINFO_FILE}${NC}"
    FOUND_ANY=true
fi

# Update desktop & icon caches
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
fi

# Check for --purge flag
PURGE=false
for arg in "$@"; do
    case "${arg}" in
        --purge|-p)
            PURGE=true
            ;;
    esac
done

if [ "${PURGE}" = true ]; then
    rm -rf "${CONFIG_DIR}" "${CACHE_DIR}" "${DATA_DIR}"
    echo -e "${GREEN}✓ Removed user configuration and cache directories (${CONFIG_DIR}, ${CACHE_DIR}).${NC}"
else
    if [ -d "${CONFIG_DIR}" ] || [ -d "${CACHE_DIR}" ]; then
        echo -e "${YELLOW}[NOTE] Project config (${CONFIG_DIR}) and thumbnail cache (${CACHE_DIR}) preserved.${NC}"
        echo -e "To purge user config and cache as well, run: ${BOLD}uninstall.sh --purge${NC}"
    fi
fi

if [ "${FOUND_ANY}" = true ]; then
    echo ""
    echo -e "${GREEN}${BOLD}✓ FluxCut has been successfully uninstalled from your system.${NC}"
else
    echo ""
    echo -e "${YELLOW}[INFO] FluxCut files were not found in standard user paths (~/.local/bin, ~/.cargo/bin).${NC}"
fi
