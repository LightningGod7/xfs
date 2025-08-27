# xfs: Firmware to Root Filesystem Extractor

`xfs` is a fork of [fw2tar](https://github.com/rehosting/fw2tar) modified for integration with the xEndity project. It is an _unprivileged_ utility designed to extract firmware images, identify root filesystems, and produce both compressed archives and extracted directory structures.

## Fork Purpose

This fork was created specifically for integration with the [xEndity](https://github.com/xendity) project, which requires:

1. A standardized output structure for firmware analysis
2. Preservation of extracted files for further analysis
3. Consistent output paths and reporting for web GUI integration
4. Simplified command-line interface with sensible defaults

## Changes from Original fw2tar

The following modifications have been made to the original fw2tar project:

1. **Renamed binary and labels**:
   - Renamed from `fw2tar` to `xfs`
   - Updated all user-facing labels while preserving internal symbols for easier upstream updates

2. **Modified output behavior**:
   - Keeps the scratch directory by default
   - Saves all default outputs to current directory if no output options specified:
     - `rootfs.tar.gz`: Compressed root filesystem archive
     - `rootfs/`: Copy of the identified root filesystem (when `--copy-rootfs` is used)
     - `xfs-extract/`: Preserved extraction directory with all extracted files

3. **Updated command-line arguments**:
   - `--output`: Now specifies the output directory for all artifacts
   - Removed `--scratch-dir` (scratch directory is saved by default)
   - Added `--no-scratch`: Boolean option to disable scratch directory saving
   - Added `--copy-rootfs`: Option to copy the identified rootfs directory
   - Added `--progress`: Option to show detailed extraction progress

4. **Improved output reporting**:
   - Outputs the relative path to the identified rootfs directory
   - Reports paths relative to the execution environment (`./xfs-extract`, `./rootfs`, `./rootfs.tar.gz`)
   - Structured progress output for web GUI integration
   - Default output shows only final results in a specific order

5. **Enhanced error handling**:
   - Combined existence check for `rootfs.tar.gz` and `xfs-extract` directory
   - Improved `--force` option to handle both files and directories

## Usage

Once installed, extracting a firmware is as simple as:

```
xfs /path/to/your/firmware.bin
```

This will generate:
- `./rootfs.tar.gz`: Compressed archive of the root filesystem
- `./xfs-extract/`: Directory containing all extracted files
- When using `--copy-rootfs`: `./rootfs/`: Copy of the identified root filesystem

### Common Options

```
xfs /path/to/firmware.bin [OPTIONS]

OPTIONS:
  --output PATH       Specify output directory for all artifacts
  --copy-rootfs       Copy the identified rootfs directory
  --no-scratch        Don't preserve extraction directory
  --force             Overwrite existing output files/directories
  --progress          Show detailed extraction progress
  --help              Show help information
```

There are two types of arguments, wrapper arguments (which handle anything outside of the xfs docker container) and xfs flags (which get passed to the actual application). These can be found with `--wrapper-help` and `--help` respectively.

### Installing Pre-built

#### Download the container
Ensure Docker is installed on your system, then download the container from GitHub:

```sh
docker pull xendity/xfs:latest
```

### Install the Wrapper

Run the following command:

```
docker run xendity/xfs
```

It will give you a command for installing system-wide or for your individual user. Run the command for your preferred install type, then follow any additional instructions from that command.

### Docker from source

Ensure you have Git and Docker installed, then:

#### Clone and build the container

```sh
git clone https://github.com/xendity/xfs.git
cd xfs
./xfs --build
```

If you wish to install globally, see "Install the Wrapper" above.

## Rebasing After Upstream Updates

When the original fw2tar project is updated, you may want to incorporate those changes into this fork. Here's how to rebase your changes:

1. **Add the upstream remote**:
   ```sh
   git remote add upstream https://github.com/rehosting/fw2tar.git
   git fetch upstream
   ```

2. **Create a temporary branch for rebasing**:
   ```sh
   git checkout -b rebase-temp
   git rebase upstream/main
   ```

3. **Resolve conflicts**:
   - Focus on preserving the following changes:
     - Renamed binary and user-facing labels
     - Modified output behavior and directory structure
     - Updated command-line arguments
     - Improved output reporting
     - Enhanced error handling
   - Internal code that doesn't affect these features can be updated from upstream

4. **Test thoroughly after rebasing**:
   ```sh
   cargo build --release
   docker build -t xendity/xfs .
   ```

5. **Update the main branch**:
   ```sh
   git checkout main
   git merge rebase-temp
   git branch -d rebase-temp
   ```

## Guidelines for Future Changes

When implementing new features or making changes to this fork, follow these guidelines:

1. **Maintain compatibility with upstream**:
   - Keep internal symbols named `fw2tar*` to simplify future rebases
   - Isolate xEndity-specific changes to clearly identifiable sections

2. **Follow consistent output conventions**:
   - Use relative paths starting with `./` for user-facing output
   - Maintain the standard output directory structure
   - Ensure progress reporting is consistent with the established format

3. **Preserve error handling patterns**:
   - Follow the existing error handling patterns
   - Use the `Fw2tarError` enum for new error types

4. **Testing changes**:
   - Test with various firmware types
   - Verify both Docker and native execution
   - Ensure the web GUI integration still works

5. **Documentation**:
   - Update this README.md with any new features or changes
   - Document any new command-line arguments
   - Keep the usage examples current

## Original fw2tar Information

For information about the original project, including its features, extractors, and distribution statements, please see the [fw2tar repository](https://github.com/rehosting/fw2tar).

# Distribution

DISTRIBUTION STATEMENT A. Approved for public release. Distribution is unlimited.
 
This material is based upon work supported under Air Force Contract No. FA8702-15-D-0001 or FA8702-25-D-B002. Any opinions, findings, conclusions or recommendations expressed in this material are those of the author(s) and do not necessarily reflect the views of the U.S. Air Force.
 
© 2025 Massachusetts Institute of Technology
 
The software/firmware is provided to you on an As-Is basis.
 
Delivered to the U.S. Government with Unlimited Rights, as defined in DFARS Part 252.227-7013 or 7014 (Feb 2014). Notwithstanding any copyright notice, U.S. Government rights in this work are defined by DFARS 252.227-7013 or DFARS 252.227-7014 as detailed above. Use of this work other than as specifically authorized by the U.S. Government may violate any copyrights that exist in this work.