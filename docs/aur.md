# AUR Package Submission Guide for waystt

This document provides comprehensive instructions for submitting waystt to the Arch User Repository (AUR).

## Prerequisites

### 1. System Requirements
```bash
# Install base development tools
sudo pacman -S base-devel git

# Ensure you have an active AUR account
# Register at: https://aur.archlinux.org/register/
```

### 2. SSH Authentication Setup

**Generate SSH key for AUR:**
```bash
# Generate SSH key pair (if you don't have one)
ssh-keygen -t rsa -b 4096 -C "your_email@example.com"

# Add public key to AUR account
cat ~/.ssh/id_rsa.pub
# Copy the output and paste it in your AUR profile settings
```

**Configure SSH for AUR:**
```bash
# Add to ~/.ssh/config
Host aur.archlinux.org
  IdentityFile ~/.ssh/id_rsa
  User aur
```

## Package Creation Process

### 1. Choose Package Strategy

**Option A: Source Package (Recommended)**
- Package name: `waystt`
- Builds from source using cargo
- Always up-to-date with latest changes

**Option B: Binary Package**
- Package name: `waystt-bin`
- Uses prebuilt GitHub release binary
- Faster installation, less flexible

### 2. Clone AUR Repository

```bash
# For source package
git clone ssh://aur@aur.archlinux.org/waystt.git
cd waystt

# For binary package
git clone ssh://aur@aur.archlinux.org/waystt-bin.git
cd waystt-bin
```

### 3. Create PKGBUILD (Source Package)

Create `PKGBUILD` file:

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=waystt
pkgver=0.1.0
pkgrel=1
pkgdesc="Signal-driven speech-to-text tool for Wayland"
arch=('x86_64')
url="https://github.com/sevos/waystt"
license=('GPL-3.0-or-later')
depends=('pipewire' 'ydotool' 'wtype' 'gcc-libs' 'glibc')
makedepends=('cargo' 'git' 'pkg-config' 'alsa-lib' 'pipewire' 'jack2')
options=('!lto')
source=("$pkgname-$pkgver.tar.gz::https://github.com/sevos/waystt/archive/v$pkgver.tar.gz")
b2sums=('SKIP')  # Update with actual checksums

prepare() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --all-features
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm0755 -t "$pkgdir/usr/bin/" "target/release/$pkgname"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
    install -Dm644 CHANGELOG.md "$pkgdir/usr/share/doc/$pkgname/CHANGELOG.md"
}
```

### 4. Create PKGBUILD (Binary Package)

Alternative `PKGBUILD` for binary package:

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=waystt-bin
_pkgname=waystt
pkgver=0.1.0
pkgrel=1
pkgdesc="Signal-driven speech-to-text tool for Wayland (binary release)"
arch=('x86_64')
url="https://github.com/sevos/waystt"
license=('GPL-3.0-or-later')
depends=('pipewire' 'ydotool' 'wtype' 'gcc-libs' 'glibc')
provides=("$_pkgname")
conflicts=("$_pkgname")
source=("$_pkgname-$pkgver::https://github.com/sevos/waystt/releases/download/v$pkgver/waystt-linux-x86_64"
        "LICENSE::https://raw.githubusercontent.com/sevos/waystt/v$pkgver/LICENSE"
        "README.md::https://raw.githubusercontent.com/sevos/waystt/v$pkgver/README.md")
b2sums=('SKIP' 'SKIP' 'SKIP')  # Update with actual checksums
noextract=("$_pkgname-$pkgver")

package() {
    install -Dm0755 "$_pkgname-$pkgver" "$pkgdir/usr/bin/$_pkgname"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
```

## Testing and Validation

### 1. Generate Checksums
```bash
# Update checksums automatically (recommended)
updpkgsums

# Or manually calculate
b2sum source_file.tar.gz
```

### 2. Test Build
```bash
# Test the package build
makepkg

# Test installation
sudo pacman -U waystt-*.pkg.tar.xz

# Test the application
waystt --help
```

### 3. Generate .SRCINFO
```bash
# Generate metadata file (required for AUR)
makepkg --printsrcinfo > .SRCINFO
```

## Submission Process

### 1. Commit and Push
```bash
# Add files to git
git add PKGBUILD .SRCINFO

# Commit with descriptive message
git commit -m "Initial release of waystt v0.1.0"

# Push to AUR
git push origin master
```

### 2. Verify Submission
- Check package appears on AUR website
- Verify package information is correct
- Test installation from AUR using an AUR helper

## Maintenance

### Updating for New Releases

```bash
# Update version in PKGBUILD
pkgver=0.2.0
pkgrel=1  # Reset to 1 for new version

# Update checksums
updpkgsums

# Regenerate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# Test build
makepkg

# Commit and push
git add PKGBUILD .SRCINFO
git commit -m "Update to v0.2.0"
git push origin master
```

### Responding to Issues
- Monitor AUR comments and feedback
- Fix build issues promptly
- Update dependencies as needed
- Maintain compatibility with Arch Linux updates

## Automated Tools

### cargo-aur
Automatically generate PKGBUILD from Cargo.toml:

```bash
# Install cargo-aur
cargo install cargo-aur

# Generate PKGBUILD and tarball
cargo aur

# This creates:
# - A release tarball
# - A complete PKGBUILD
# - All necessary metadata
```

### cargo-pkgbuild
Alternative PKGBUILD generator:

```bash
# Install
cargo install cargo-pkgbuild

# Generate PKGBUILD
cargo pkgbuild
```

## Common Issues and Solutions

### Build Dependencies
- Ensure all system libraries are listed in `makedepends`
- For waystt: `alsa-lib`, `pipewire`, `jack2` for audio support
- `pkg-config` for build configuration

### Runtime Dependencies
- List actual runtime requirements in `depends`
- For waystt: `pipewire`, `ydotool`, `wtype` are essential
- `gcc-libs` and `glibc` for basic C library support

### License Compatibility
- Ensure license field matches actual project license
- GPL-3.0-or-later requires source disclosure
- Include LICENSE file in package

### Network Access
- Use `prepare()` function for `cargo fetch`
- Build function should work offline with `--frozen`
- This follows Arch packaging guidelines

## Best Practices

1. **Naming Convention**
   - Source packages: Use upstream name directly
   - Binary packages: Add `-bin` suffix
   - Development packages: Add `-git` suffix

2. **Version Management**
   - Follow semantic versioning
   - Reset `pkgrel` to 1 for new `pkgver`
   - Increment `pkgrel` for package fixes

3. **Documentation**
   - Include README and LICENSE files
   - Add changelog for significant updates
   - Maintain clear commit messages

4. **Community Engagement**
   - Respond to user feedback
   - Fix reported issues promptly
   - Consider suggestions for improvements

## Additional Resources

- [AUR Submission Guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines)
- [PKGBUILD Manual](https://wiki.archlinux.org/title/PKGBUILD)
- [Rust Package Guidelines](https://wiki.archlinux.org/title/Rust_package_guidelines)
- [Creating Packages](https://wiki.archlinux.org/title/Creating_packages)