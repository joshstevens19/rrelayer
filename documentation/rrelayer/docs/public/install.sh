#!/usr/bin/env bash
set -eo pipefail

BASE_DIR="${XDG_CONFIG_HOME:-$HOME}"
RRELAYER_DIR="${RRELAYER_DIR:-"$BASE_DIR/.rrelayer"}"
RRELAYER_BIN_DIR="$RRELAYER_DIR/bin"
RRELAYERUP_PATH="$RRELAYER_BIN_DIR/rrelayerup"
RRELAYERDOWN_PATH="$RRELAYER_BIN_DIR/rrelayerdown"
OS_TYPE=$(uname)
ARCH_TYPE=$(uname -m)

# Parse command line arguments
VERSION=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --local)
            LOCAL_INSTALL=true
            shift
            ;;
        --uninstall)
            UNINSTALL=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

if [[ "$OS_TYPE" == "Linux" ]]; then
    BIN_PATH="$RRELAYER_BIN_DIR/rrelayer"
    PLATFORM="linux"
    ARCH_TYPE="amd64"
    EXT="tar.gz"
    if ! command -v unzip &> /dev/null; then
        sudo apt-get update && sudo apt-get install -y unzip
    fi
elif [[ "$OS_TYPE" == "Darwin" ]]; then
    BIN_PATH="$RRELAYER_BIN_DIR/rrelayer"
    PLATFORM="darwin"
    if [[ "$ARCH_TYPE" == "x86_64" ]]; then
        ARCH_TYPE="amd64"
    fi
    EXT="tar.gz"
elif [[ "$OS_TYPE" == "MINGW"* ]] || [[ "$OS_TYPE" == "MSYS"* ]] || [[ "$OS_TYPE" == "CYGWIN"* ]]; then
    PLATFORM="win32"
    EXT="zip"
    BIN_PATH="$RRELAYER_BIN_DIR/rrelayer.exe"
else
    echo "Unsupported OS: $OS_TYPE"
    exit 1
fi

# Function to get the latest version from GitHub API
get_latest_version() {
    local latest_version
    latest_version=$(curl -s "https://api.github.com/repos/joshstevens19/rrelayer/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//')
    echo "$latest_version"
}

# Set download URLs
if [[ -n "$VERSION" ]]; then
    BIN_URL="https://github.com/joshstevens19/rrelayer/releases/download/v${VERSION}/rrelayer_${PLATFORM}-${ARCH_TYPE}.${EXT}"
else
    # Get latest version if no version specified
    VERSION=$(get_latest_version)
    BIN_URL="https://github.com/joshstevens19/rrelayer/releases/download/v${VERSION}/rrelayer_${PLATFORM}-${ARCH_TYPE}.${EXT}"
fi

log() {
   echo -e "\033[1;32m$1\033[0m"
}

error_log() {
    echo -e "\033[1;31m$1\033[0m"
}

spinner() {
    local text="$1"
    local pid=$!
    local delay=0.1
    local spinstr='|/-\'
    log "$text"
    while ps -p "$pid" &>/dev/null; do
        local temp=${spinstr#?}
        printf " [%c]  " "$spinstr"
        local spinstr=$temp${spinstr%"$temp"}
        sleep $delay
        printf "\b\b\b\b\b\b"
    done
    echo ""
}

# Install or uninstall based on the command line option
if [[ "$LOCAL_INSTALL" == true ]]; then
    log "Using local binary from $LOCAL_BIN_PATH..."
    mkdir -p "$RRELAYER_BIN_DIR"
    cp "$LOCAL_BIN_PATH" "$BIN_PATH"
elif [[ "$UNINSTALL" == true ]]; then
    log "Uninstalling rrelayer..."
    rm -f "$BIN_PATH" "$RRELAYERUP_PATH" "$RRELAYERDOWN_PATH"
    rmdir "$RRELAYER_BIN_DIR" "$RRELAYER_DIR" 2> /dev/null
    log "Uninstallation complete! Please restart your shell or source your profile to complete the process."
    exit 0
else
    if [[ -n "$VERSION" ]]; then
        log "Installing rrelayer version $VERSION..."
    else
        log "Installing latest rrelayer version ($VERSION)..."
    fi
    log "Preparing the installation..."
    mkdir -p "$RRELAYER_BIN_DIR"
    log "Downloading binary archive from $BIN_URL..."

    if ! curl -sSf -L "$BIN_URL" -o "$RRELAYER_DIR/rrelayer.${EXT}"; then
        error_log "Failed to download rrelayer version $VERSION. Please check if the version exists."
        error_log "Available releases: https://github.com/joshstevens19/rrelayer/releases"
        exit 1
    fi

    log "Downloaded binary archive to $RRELAYER_DIR/rrelayer.${EXT}"

    log "Extracting archive..."
    if [[ "$EXT" == "tar.gz" ]]; then
        tar -xzvf "$RRELAYER_DIR/rrelayer.${EXT}" -C "$RRELAYER_DIR"
    else
        unzip -o "$RRELAYER_DIR/rrelayer.${EXT}" -d "$RRELAYER_DIR"
    fi

    # Move the main binary to the bin directory, creating it if necessary
    mkdir -p "$RRELAYER_BIN_DIR"
    if [[ -f "$RRELAYER_DIR/rrelayer_cli" ]]; then
        mv "$RRELAYER_DIR/rrelayer_cli" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer" ]]; then
        mv "$RRELAYER_DIR/rrelayer" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer_cli.exe" ]]; then
        mv "$RRELAYER_DIR/rrelayer_cli.exe" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer.exe" ]]; then
        mv "$RRELAYER_DIR/rrelayer.exe" "$BIN_PATH"
    fi

    log "Extracted files to $RRELAYER_DIR"

    rm "$RRELAYER_DIR/rrelayer.${EXT}"
fi

# Ensure the binary exists before setting permissions
if [ -f "$BIN_PATH" ]; then
    chmod +x "$BIN_PATH"
    log "Binary found and permissions set at $BIN_PATH"
else
    error_log "Error: Binary not found at $BIN_PATH"
    exit 1
fi

# Update PATH in user's profile
PROFILE="${HOME}/.profile"  # Default to .profile
case $SHELL in
    */zsh) PROFILE="${ZDOTDIR:-"$HOME"}/.zshenv" ;;
    */bash) PROFILE="$HOME/.bashrc" ;;
    */fish) PROFILE="$HOME/.config/fish/config.fish" ;;
esac

if [[ ":$PATH:" != *":${RRELAYER_BIN_DIR}:"* ]] && ! grep -q "$RRELAYER_BIN_DIR" "$PROFILE"; then
    echo "export PATH=\"\$PATH:$RRELAYER_BIN_DIR\"" >> "$PROFILE"
    log "PATH updated in $PROFILE. Please log out and back in or source the profile file."
fi

# rrelayerup and rrelayerdown are now executable scripts in PATH, no aliases needed

# Create or update the rrelayerup script to check for updates
cat <<EOF > "$RRELAYERUP_PATH"
#!/usr/bin/env bash
set -eo pipefail

# Parse command line arguments for rrelayerup
UPDATE_VERSION=""
while [[ \$# -gt 0 ]]; do
    case \$1 in
        --version)
            UPDATE_VERSION="\$2"
            shift 2
            ;;
        --local)
            LOCAL_UPDATE=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

echo "Updating rrelayer..."
if [[ "\$LOCAL_UPDATE" == true ]]; then
    echo "Using local binary for update..."
    cp "$LOCAL_BIN_PATH" "$BIN_PATH"
else
    # Function to get the latest version from GitHub API
    get_latest_version() {
        local latest_version
        latest_version=\$(curl -s "https://api.github.com/repos/joshstevens19/rrelayer/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//')
        echo "\$latest_version"
    }

    # Set version for update
    if [[ -n "\$UPDATE_VERSION" ]]; then
        echo "Updating to rrelayer version \$UPDATE_VERSION..."
        DOWNLOAD_URL="https://github.com/joshstevens19/rrelayer/releases/download/v\${UPDATE_VERSION}/rrelayer_${PLATFORM}-${ARCH_TYPE}.${EXT}"
    else
        UPDATE_VERSION=\$(get_latest_version)
        echo "Updating to latest rrelayer version (\$UPDATE_VERSION)..."
        DOWNLOAD_URL="https://github.com/joshstevens19/rrelayer/releases/download/v\${UPDATE_VERSION}/rrelayer_${PLATFORM}-${ARCH_TYPE}.${EXT}"
    fi

    echo "Downloading the binary from \$DOWNLOAD_URL..."
    if ! curl -sSf -L "\$DOWNLOAD_URL" -o "$RRELAYER_DIR/rrelayer.${EXT}"; then
        echo "Failed to download rrelayer version \$UPDATE_VERSION. Please check if the version exists."
        echo "Available releases: https://github.com/joshstevens19/rrelayer/releases"
        exit 1
    fi

    echo "Extracting archive..."
    if [[ "$EXT" == "tar.gz" ]]; then
        tar -xzvf "$RRELAYER_DIR/rrelayer.${EXT}" -C "$RRELAYER_DIR"
    else
        unzip -o "$RRELAYER_DIR/rrelayer.${EXT}" -d "$RRELAYER_DIR"
    fi

    # Move the main binary to the bin directory
    if [[ -f "$RRELAYER_DIR/rrelayer_cli" ]]; then
        mv "$RRELAYER_DIR/rrelayer_cli" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer" ]]; then
        mv "$RRELAYER_DIR/rrelayer" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer_cli.exe" ]]; then
        mv "$RRELAYER_DIR/rrelayer_cli.exe" "$BIN_PATH"
    elif [[ -f "$RRELAYER_DIR/rrelayer.exe" ]]; then
        mv "$RRELAYER_DIR/rrelayer.exe" "$BIN_PATH"
    fi

    rm "$RRELAYER_DIR/rrelayer.${EXT}"
fi
chmod +x "$BIN_PATH"
echo "rrelayer has been updated successfully."
EOF

chmod +x "$RRELAYERUP_PATH"

# rrelayerdown
cat <<EOF > "$RRELAYERDOWN_PATH"
#!/usr/bin/env bash
set -eo pipefail

echo "Uninstalling rrelayer..."
rm -f "$BIN_PATH" "$RRELAYERUP_PATH" "$RRELAYERDOWN_PATH"
rmdir "$RRELAYER_BIN_DIR" "$RRELAYER_DIR" 2> /dev/null
echo "rrelayer uninstallation complete!"
echo "Note: You may need to restart your shell or source your profile to update PATH."
EOF

chmod +x "$RRELAYERDOWN_PATH"

log ""
log "rrelayer has been installed successfully"
log ""
log "To update rrelayer run 'rrelayerup' (latest) or 'rrelayerup --version X.X.X' (specific version)."
log ""
log "To uninstall rrelayer run 'rrelayerdown'."
log ""
log "Open a new terminal and run 'rrelayer' to get started."