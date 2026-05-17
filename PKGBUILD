# Maintainer: Evanga

pkgname=sruler-git
_pkgname=sruler
pkgver=r0.0000000
pkgrel=1
pkgdesc="Simple Wayland ruler"
arch=('x86_64')
url="https://github.com/amekhan1a/sruler"
license=('GPL3')
depends=('grim' 'gcc-libs')
makedepends=('cargo' 'git' 'rust')
source=("git+$url.git")
sha256sums=('SKIP')

pkgver() {
    cd "$srcdir/$_pkgname"
    printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

build() {
    cd "$srcdir/$_pkgname"
    cargo build --release --locked
}

package() {
    cd "$srcdir/$_pkgname"

    install -Dm755 "target/release/sruler" "$pkgdir/usr/bin/sruler"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
