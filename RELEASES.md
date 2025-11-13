# Release Process

This repository uses GitHub Actions to automatically build and publish releases for multiple platforms.

## Creating a Release

To create a new release, simply bump the version in `Cargo.toml` and push to the main branch:

1. Update the version in `Cargo.toml`:
   ```toml
   [package]
   version = "1.0.0"  # Update this line
   ```

2. Commit and push to main:
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to 1.0.0"
   git push origin main
   ```

The GitHub Actions workflows will automatically:
- Detect the version change in `Cargo.toml`
- Create a git tag (e.g., `v1.0.0`) if it doesn't already exist
- Trigger the release workflow
- Build binaries for all supported platforms (Linux, Windows, macOS ARM)
- Create a new GitHub release with the tag
- Attach all binaries to the release
- Generate release notes from commits

### Manual Tag Creation (Alternative)

You can also manually create and push a version tag:
```bash
git tag v1.0.0
git push origin v1.0.0
```

This will skip the auto-tagging workflow and directly trigger the release build.

## Supported Platforms

The release workflow builds binaries for:

- **Linux x86_64**: Statically linked using musl for maximum compatibility
- **Windows x86_64**: Built with MSVC and static CRT
- **macOS ARM64**: Built for Apple Silicon Macs

## Binary Naming

Released binaries follow this naming convention:
- `systemair-save-tools-linux-x86_64` - Linux binary
- `systemair-save-tools-windows-x86_64.exe` - Windows binary
- `systemair-save-tools-macos-arm64` - macOS ARM64 binary

## Manual Workflow Trigger

The workflow can also be triggered manually from the Actions tab in GitHub:
1. Go to the "Actions" tab
2. Select "Release" workflow
3. Click "Run workflow"
4. Select the branch and run

Note: Manual triggers will build the binaries but won't create a release unless run from a tag.

## Static Linking

All binaries are built with static linking preferences:
- **Linux**: Uses musl target with `crt-static` for fully static binaries
- **Windows**: Uses `crt-static` target feature for static CRT
- **macOS**: Binaries are stripped for smaller size

This ensures the binaries are as portable as possible with minimal dependencies.
