#!/bin/bash
# Harness-Gate 安装脚本

set -e

REPO="musutrade/Harness-Gate"
BINARY_NAME="harness-gate"

# 检测操作系统和架构
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="macos"
            ;;
        mingw*|msys*|cygwin*)
            OS="windows"
            ;;
        *)
            echo "不支持的操作系统: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)
            ARCH="amd64"
            ;;
        aarch64|arm64)
            ARCH="arm64"
            ;;
        *)
            echo "不支持的架构: $arch"
            exit 1
            ;;
    esac

    PLATFORM="${OS}-${ARCH}"
}

# 获取最新版本
get_latest_version() {
    echo "正在获取最新版本..."
    VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        echo "无法获取最新版本，请检查网络连接或仓库地址"
        exit 1
    fi

    echo "最新版本: $VERSION"
}

# 下载二进制文件
download_binary() {
    detect_platform

    local ext=""
    if [ "$OS" = "windows" ]; then
        ext=".exe"
    fi

    local filename="${BINARY_NAME}-${PLATFORM}${ext}"
    local url="https://github.com/${REPO}/releases/download/${VERSION}/${filename}"

    echo "正在下载 $filename..."

    if command -v curl &> /dev/null; then
        curl -L -o "$BINARY_NAME" "$url"
    elif command -v wget &> /dev/null; then
        wget -O "$BINARY_NAME" "$url"
    else
        echo "错误: 需要 curl 或 wget"
        exit 1
    fi

    chmod +x "$BINARY_NAME"
}

# 安装二进制文件
install_binary() {
    local install_dir="${INSTALL_DIR:-$HOME/.local/bin}"

    echo "正在安装到 $install_dir..."

    mkdir -p "$install_dir"
    mv "$BINARY_NAME" "$install_dir/"

    echo ""
    echo "✅ Harness-Gate 安装成功！"
    echo ""
    echo "请确保 $install_dir 在你的 PATH 中。"
    echo "如果还没有添加，请运行:"
    echo ""
    echo "  export PATH=\"$install_dir:\$PATH\""
    echo ""
    echo "并将其添加到你的 shell 配置文件 (~/.bashrc 或 ~/.zshrc)"
    echo ""
    echo "现在你可以运行:"
    echo "  harness-gate --version"
}

# 从源码安装
install_from_source() {
    echo "正在从源码安装..."

    if ! command -v cargo &> /dev/null; then
        echo "错误: 需要 Rust 工具链。请访问 https://rustup.rs/ 安装"
        exit 1
    fi

    if [ ! -d ".git" ]; then
        echo "正在克隆仓库..."
        git clone "https://github.com/${REPO}.git" harness-gate-src
        cd harness-gate-src
    fi

    echo "正在编译..."
    cargo install --locked --path tools/harness-gate

    echo ""
    echo "✅ Harness-Gate 从源码安装成功！"
    echo ""
    echo "现在你可以运行:"
    echo "  harness-gate --version"
}

# 主函数
main() {
    echo "=== Harness-Gate 安装程序 ==="
    echo ""

    if [ "$1" = "--from-source" ]; then
        install_from_source
    else
        get_latest_version
        download_binary
        install_binary
    fi
}

main "$@"
