#!/bin/sh
# opi installer.
#
#   curl -fsSL https://raw.githubusercontent.com/casoon/opi/main/install.sh | sh
#
# Downloads the release binary for this platform, checks it against the
# published SHA-256, and puts it on your PATH. Nothing else is touched.
#
#   OPI_VERSION      version to install, without the leading v (default: latest)
#   OPI_INSTALL_DIR  where the binary goes (default: $HOME/.local/bin)

set -eu

REPO="casoon/opi"
INSTALL_DIR="${OPI_INSTALL_DIR:-$HOME/.local/bin}"

say() {
	printf '%s\n' "$1"
}

# An error names what happened and what to do about it, the way opi's own
# errors do.
fail() {
	printf '\nopi: %s\n' "$1" >&2
	if [ "$#" -gt 1 ]; then
		printf '  %s\n' "$2" >&2
	fi
	printf '\n' >&2
	exit 1
}

fetch() {
	# fetch <url> <destination>, or <url> alone to print to stdout.
	if [ "$#" -eq 2 ]; then
		if [ -n "$HAVE_CURL" ]; then
			curl -fsSL "$1" -o "$2"
		else
			wget -qO "$2" "$1"
		fi
	else
		if [ -n "$HAVE_CURL" ]; then
			curl -fsSL "$1"
		else
			wget -qO- "$1"
		fi
	fi
}

target() {
	os="$(uname -s)"
	arch="$(uname -m)"
	case "$os" in
	Darwin)
		case "$arch" in
		arm64 | aarch64) say "aarch64-apple-darwin" ;;
		x86_64) say "x86_64-apple-darwin" ;;
		*) fail "no release binary for $os $arch." "Build it yourself: cargo install opi" ;;
		esac
		;;
	Linux)
		case "$arch" in
		x86_64 | amd64) say "x86_64-unknown-linux-musl" ;;
		aarch64 | arm64) say "aarch64-unknown-linux-musl" ;;
		*) fail "no release binary for $os $arch." "Build it yourself: cargo install opi" ;;
		esac
		;;
	*)
		fail "opi is Unix only, and this is $os." "It runs scripts with exec and drives termios directly; neither exists on Windows."
		;;
	esac
}

latest_version() {
	# The /releases/latest URL redirects to the newest tag, which avoids the
	# rate limit an unauthenticated API call runs into.
	if [ -n "$HAVE_CURL" ]; then
		url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")"
	else
		url="$(wget -qS --spider "https://github.com/$REPO/releases/latest" 2>&1 |
			awk '/^[ \t]*Location:/ { print $2 }' | tail -n 1)"
	fi
	tag="${url##*/}"
	case "$tag" in
	v*) say "${tag#v}" ;;
	*) fail "could not read the latest version from GitHub." "Pick one yourself: OPI_VERSION=0.7.0 sh install.sh" ;;
	esac
}

checksum() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d ' ' -f 1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d ' ' -f 1
	else
		fail "no sha256sum or shasum on this system." "Install coreutils, or download the release by hand from https://github.com/$REPO/releases"
	fi
}

HAVE_CURL=""
if command -v curl >/dev/null 2>&1; then
	HAVE_CURL="yes"
elif ! command -v wget >/dev/null 2>&1; then
	fail "neither curl nor wget is installed." "Install one of them and run this again."
fi
command -v tar >/dev/null 2>&1 || fail "tar is not installed." "Install it and run this again."

TARGET="$(target)"
VERSION="${OPI_VERSION:-$(latest_version)}"
NAME="opi-${VERSION}-${TARGET}"
URL="https://github.com/$REPO/releases/download/v${VERSION}/${NAME}.tar.gz"

say "opi ${VERSION} · ${TARGET}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

fetch "$URL" "$TMP/$NAME.tar.gz" ||
	fail "no download at $URL." "Check which versions exist: https://github.com/$REPO/releases"
fetch "$URL.sha256" "$TMP/$NAME.tar.gz.sha256" ||
	fail "the checksum for ${NAME}.tar.gz is missing." "Do not install this archive unverified; report it at https://github.com/$REPO/issues"

expected="$(cut -d ' ' -f 1 <"$TMP/$NAME.tar.gz.sha256")"
actual="$(checksum "$TMP/$NAME.tar.gz")"
if [ "$expected" != "$actual" ]; then
	fail "the download does not match its checksum." "Expected $expected, got $actual. Do not install it; report this at https://github.com/$REPO/issues"
fi

tar -xzf "$TMP/$NAME.tar.gz" -C "$TMP"
[ -f "$TMP/$NAME/opi" ] || fail "the archive does not contain opi." "Report this at https://github.com/$REPO/issues"

mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP/$NAME/opi" "$INSTALL_DIR/opi" 2>/dev/null ||
	fail "could not write to $INSTALL_DIR." "Pick a directory you own: OPI_INSTALL_DIR=\"\$HOME/bin\" sh install.sh"

say "installed $INSTALL_DIR/opi"

case ":$PATH:" in
*":$INSTALL_DIR:"*) ;;
*)
	say ""
	say "$INSTALL_DIR is not on your PATH. Add it:"
	say "  export PATH=\"$INSTALL_DIR:\$PATH\""
	;;
esac
