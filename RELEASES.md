# Release Process

This repository uses GitHub Actions to automatically build and publish releases for multiple platforms.

## Creating a Release

To create a new release:

1. Ensure all changes are committed and pushed to the main branch
2. Create and push a version tag:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

The GitHub Actions workflow will automatically:
- Build binaries for all supported platforms (Linux, Windows, macOS Intel, macOS ARM)
- Create a new GitHub release with the tag
- Attach all binaries to the release
- Generate release notes from commits

## Supported Platforms

The release workflow builds binaries for:

- **Linux x86_64**: Statically linked using musl for maximum compatibility
- **Windows x86_64**: Built with MSVC and static CRT
- **macOS x86_64**: Built for Intel-based Macs
- **macOS ARM64**: Built for Apple Silicon Macs

## Binary Naming

Released binaries follow this naming convention:
- `systemair-save-tools-linux-x86_64` - Linux binary
- `systemair-save-tools-windows-x86_64.exe` - Windows binary
- `systemair-save-tools-macos-x86_64` - macOS Intel binary
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
