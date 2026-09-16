#!/bin/sh
# Numeria 一键安装:识别平台,下载最新 release 的 git-numeria 放入 PATH。
#   curl -fsSL https://raw.githubusercontent.com/omeyang/Numeria/main/install.sh | sh
# 可选环境变量:
#   NUMERIA_VERSION      指定版本(默认最新,如 v0.0.1)
#   NUMERIA_INSTALL_DIR  安装目录(默认 ~/.local/bin,root 为 /usr/local/bin)
set -eu

REPO="omeyang/Numeria"
BIN="git-numeria"

err() { printf 'install: %s\n' "$1" >&2; exit 1; }

os=$(uname -s)
arch=$(uname -m)
case "$os-$arch" in
  Linux-x86_64)           target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64)           target=aarch64-apple-darwin ;;
  Darwin-x86_64)          target=x86_64-apple-darwin ;;
  *) err "暂不支持的平台: $os $arch(Windows 请从 Releases 下载 zip)" ;;
esac

if [ -n "${NUMERIA_VERSION:-}" ]; then
  version=$NUMERIA_VERSION
else
  resp=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest") || err "获取最新版本失败"
  version=$(printf '%s\n' "$resp" | grep -m1 '"tag_name"' | cut -d'"' -f4)
fi
[ -n "$version" ] || err "无法确定版本号"

if [ -n "${NUMERIA_INSTALL_DIR:-}" ]; then
  dir=$NUMERIA_INSTALL_DIR
elif [ "$(id -u)" = 0 ]; then
  dir=/usr/local/bin
else
  dir=$HOME/.local/bin
fi
mkdir -p "$dir"

name="$BIN-$version-$target"
url="https://github.com/$REPO/releases/download/$version/$name.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf '下载 %s ...\n' "$url"
curl -fsSL "$url" -o "$tmp/pkg.tar.gz" || err "下载失败(该版本可能没有 $target 产物)"
tar xzf "$tmp/pkg.tar.gz" -C "$tmp"
install -m 755 "$tmp/$name/$BIN" "$dir/$BIN"

# man 页:git numeria --help / git help numeria 依赖它
if [ -f "$tmp/$name/$BIN.1" ]; then
  if [ "$(id -u)" = 0 ]; then
    mandir=/usr/local/share/man/man1
  else
    mandir=$HOME/.local/share/man/man1
  fi
  mkdir -p "$mandir"
  install -m 644 "$tmp/$name/$BIN.1" "$mandir/$BIN.1"
  printf '已安装 man 页 -> %s/%s.1(git numeria --help 可用)\n' "$mandir" "$BIN"
fi

printf '已安装 %s %s -> %s/%s\n' "$BIN" "$version" "$dir" "$BIN"
case ":$PATH:" in
  *":$dir:"*) printf '现在可以直接使用:git numeria -h\n' ;;
  *) printf '注意:%s 不在 PATH 中,请加入后使用。\n' "$dir" ;;
esac
