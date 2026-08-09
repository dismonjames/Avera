#!/bin/sh
set -e

# Avera Installation Script
# usage: curl -sSL https://raw.githubusercontent.com/dismonjames/Avera/main/install.sh | sh

GITHUB_REPO="dismonjames/Avera"
API_URL="https://api.github.com/repos/$GITHUB_REPO/releases/latest"

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" = "Linux" ]; then
    OS_NAME="linux"
elif [ "$OS" = "Darwin" ]; then
    OS_NAME="macos"
else
    echo "Unsupported OS: $OS"
    exit 1
fi

if [ "$ARCH" = "x86_64" ]; then
    ARCH_NAME="amd64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    ARCH_NAME="arm64"
else
    echo "Unsupported Architecture: $ARCH"
    exit 1
fi

ASSET_NAME="avera-${OS_NAME}-${ARCH_NAME}.tar.gz"

echo "Fetching latest release from $GITHUB_REPO..."
DOWNLOAD_URL=$(curl -s $API_URL | grep "browser_download_url.*$ASSET_NAME" | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Could not find release asset $ASSET_NAME. Check if the release exists."
    exit 1
fi

INSTALL_DIR="$HOME/.avera/bin"
mkdir -p "$INSTALL_DIR"

echo "Downloading $ASSET_NAME..."
curl -sL "$DOWNLOAD_URL" -o "/tmp/$ASSET_NAME"

echo "Extracting to $INSTALL_DIR..."
tar -xzf "/tmp/$ASSET_NAME" -C "$INSTALL_DIR"
rm "/tmp/$ASSET_NAME"

echo "Avera installed successfully at $INSTALL_DIR/avera"
echo "Please add $INSTALL_DIR to your PATH if you haven't already:"
echo ""
echo "  export PATH=\"\$HOME/.avera/bin:\$PATH\""
echo ""
