# AUR Publishing Setup

This directory contains the configuration for automated publishing of `waystt-bin` to the Arch User Repository (AUR).

## Prerequisites

### 1. AUR Account Setup
1. Create an account at [aur.archlinux.org](https://aur.archlinux.org/)
2. Generate SSH key pair for AUR access:
   ```bash
   ssh-keygen -t ed25519 -C "your-email@example.com" -f ~/.ssh/aur_key
   ```
3. Add public key to your AUR account at [AUR SSH Keys](https://aur.archlinux.org/account/)

### 2. Initial AUR Package Submission
The first submission must be done manually:
1. Clone the AUR repository:
   ```bash
   git clone ssh://aur@aur.archlinux.org/waystt-bin.git
   cd waystt-bin
   ```
2. Copy the generated PKGBUILD:
   ```bash
   # Generate PKGBUILD for current version first
   cp ../aur/waystt-bin/PKGBUILD.template PKGBUILD
   # Edit VERSION_PLACEHOLDER and CHECKSUM_PLACEHOLDER manually
   ```
3. Test the package:
   ```bash
   makepkg -si
   ```
4. Commit and push:
   ```bash
   git add PKGBUILD
   git commit -m "Initial waystt-bin package"
   git push
   ```

### 3. GitHub Repository Secrets
Add the following secrets to your GitHub repository at Settings → Secrets and variables → Actions:

- **AUR_USERNAME**: Your AUR username
- **AUR_EMAIL**: Email associated with your AUR account  
- **AUR_SSH_PRIVATE_KEY**: Content of your AUR SSH private key (cat ~/.ssh/aur_key)

## How It Works

1. **Release Trigger**: Workflow runs when a GitHub release is published
2. **Binary Download**: Downloads the pre-built `waystt-linux-x86_64` binary from the release
3. **Checksum Calculation**: Calculates SHA256 hash of the binary
4. **PKGBUILD Generation**: Creates PKGBUILD from template with current version and checksum
5. **AUR Publishing**: Uses KSXGitHub/github-actions-deploy-aur to update the AUR package

## Manual Testing

To test the PKGBUILD locally:

```bash
# Generate PKGBUILD for a specific version
cp aur/waystt-bin/PKGBUILD.template PKGBUILD
sed -i 's/VERSION_PLACEHOLDER/0.1.1/g' PKGBUILD

# Download binary and calculate checksum
wget -O waystt-linux-x86_64 "https://github.com/sevos/waystt/releases/download/v0.1.1/waystt-linux-x86_64"
CHECKSUM=$(sha256sum waystt-linux-x86_64 | cut -d' ' -f1)
sed -i "s/CHECKSUM_PLACEHOLDER/$CHECKSUM/g" PKGBUILD

# Test build
makepkg -si
```

## Troubleshooting

- **SSH Key Issues**: Ensure the private key format is correct (starts with `-----BEGIN OPENSSH PRIVATE KEY-----`)
- **Permission Denied**: Verify the AUR username and that your SSH key is added to your AUR account
- **Checksum Mismatch**: The workflow automatically calculates checksums, but manual verification might be needed for debugging
- **First Submission**: Remember that the AUR package must exist before automation can update it

## Files

- `PKGBUILD.template`: Template for generating the actual PKGBUILD
- `../.github/workflows/aur-publish.yml`: GitHub Actions workflow for automated publishing