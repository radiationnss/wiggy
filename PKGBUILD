pkgname=wiggy
pkgver=0.1.0
pkgrel=1
pkgdesc='Mouse-triggered desktop chat palette for Hyprland'
arch=('x86_64')
url='https://github.com/radiationnss/wiggy'
license=('custom')
depends=('gtk3' 'webkit2gtk-4.1' 'libappindicator')
optdepends=(
  'hyprland: mouse-wiggle summon integration'
  'grim: screen and window capture'
  'slurp: region and window selection'
)
makedepends=('nodejs' 'npm' 'rust' 'pkgconf' 'librsvg' 'gtk3' 'webkit2gtk-4.1' 'libappindicator')
options=('!debug')
source=()
sha256sums=()

build() {
  cd "$startdir"

  npm ci --no-audit --no-fund
  npm run build
  cargo build \
    --manifest-path src-tauri/Cargo.toml \
    --release \
    --locked \
    --features custom-protocol
}

package() {
  install -Dm755 "$startdir/src-tauri/target/release/wiggy" \
    "$pkgdir/usr/bin/wiggy"
  install -Dm644 "$startdir/packaging/arch/wiggy.desktop" \
    "$pkgdir/usr/share/applications/wiggy.desktop"
  install -Dm644 "$startdir/src-tauri/icons/icon.svg" \
    "$pkgdir/usr/share/icons/hicolor/scalable/apps/wiggy.svg"
}
