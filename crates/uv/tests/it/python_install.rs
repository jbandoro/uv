use std::process::Command;

use assert_fs::{assert::PathAssert, prelude::PathChild};
use predicates::prelude::predicate;
use same_file::is_same_file;

use crate::common::{uv_snapshot, TestContext};

#[test]
fn python_install() {
    let context: TestContext = TestContext::new_with_versions(&[]).with_filtered_python_keys();

    // Install the latest version
    uv_snapshot!(context.filters(), context.python_install(), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
    "###);

    let bin_python = context
        .temp_dir
        .child("bin")
        .child(format!("python3.13{}", std::env::consts::EXE_SUFFIX));

    // The executable should not be installed in the bin directory (requires preview)
    bin_python.assert(predicate::path::missing());

    // Should be a no-op when already installed
    uv_snapshot!(context.filters(), context.python_install(), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Python is already installed. Use `uv python install <request>` to install another version.
    "###);

    // Similarly, when a requested version is already installed
    uv_snapshot!(context.filters(), context.python_install().arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    "###);

    // You can opt-in to a reinstall
    uv_snapshot!(context.filters(), context.python_install().arg("--reinstall"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     ~ cpython-3.13.0-[PLATFORM]
    "###);

    // Uninstallation requires an argument
    uv_snapshot!(context.filters(), context.python_uninstall(), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: the following required arguments were not provided:
      <TARGETS>...

    Usage: uv python uninstall <TARGETS>...

    For more information, try '--help'.
    "###);

    uv_snapshot!(context.filters(), context.python_uninstall().arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Searching for Python versions matching: Python 3.13
    Uninstalled Python 3.13.0 in [TIME]
     - cpython-3.13.0-[PLATFORM]
    "###);
}

#[test]
fn python_install_preview() {
    let context: TestContext = TestContext::new_with_versions(&[]).with_filtered_python_keys();

    // Install the latest version
    uv_snapshot!(context.filters(), context.python_install().arg("--preview"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
       + python
       + python3
       + python3.13
    "###);

    let bin_python = context
        .temp_dir
        .child("bin")
        .child(format!("python3.13{}", std::env::consts::EXE_SUFFIX));

    // The executable should be installed in the bin directory
    bin_python.assert(predicate::path::exists());

    // On Unix, it should be a link
    #[cfg(unix)]
    bin_python.assert(predicate::path::is_symlink());

    // The executable should "work"
    uv_snapshot!(context.filters(), Command::new(bin_python.as_os_str())
        .arg("-c").arg("import subprocess; print('hello world')"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    hello world

    ----- stderr -----
    "###);

    // Should be a no-op when already installed
    uv_snapshot!(context.filters(), context.python_install().arg("--preview"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Python is already installed. Use `uv python install <request>` to install another version.
    "###);

    // You can opt-in to a reinstall
    uv_snapshot!(context.filters(), context.python_install().arg("--preview").arg("--reinstall"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     ~ cpython-3.13.0-[PLATFORM]
       ~ python
       ~ python3
       ~ python3.13
    "###);

    // The executable should still be present in the bin directory
    bin_python.assert(predicate::path::exists());

    // Uninstallation requires an argument
    uv_snapshot!(context.filters(), context.python_uninstall(), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: the following required arguments were not provided:
      <TARGETS>...

    Usage: uv python uninstall <TARGETS>...

    For more information, try '--help'.
    "###);

    uv_snapshot!(context.filters(), context.python_uninstall().arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Searching for Python versions matching: Python 3.13
    Uninstalled Python 3.13.0 in [TIME]
     - cpython-3.13.0-[PLATFORM]
    "###);

    // The executable should be removed
    bin_python.assert(predicate::path::missing());
}

#[test]
fn python_install_freethreaded() {
    let context: TestContext = TestContext::new_with_versions(&[]).with_filtered_python_keys();

    // Install the latest version
    uv_snapshot!(context.filters(), context.python_install().arg("--preview").arg("3.13t"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0+freethreaded-[PLATFORM]
       + python3.13t
    "###);

    let bin_python = context
        .temp_dir
        .child("bin")
        .child(format!("python3.13t{}", std::env::consts::EXE_SUFFIX));

    // The executable should be installed in the bin directory
    bin_python.assert(predicate::path::exists());

    // On Unix, it should be a link
    #[cfg(unix)]
    bin_python.assert(predicate::path::is_symlink());

    // The executable should "work"
    uv_snapshot!(context.filters(), Command::new(bin_python.as_os_str())
        .arg("-c").arg("import subprocess; print('hello world')"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    hello world

    ----- stderr -----
    "###);

    // Should be distinct from 3.13
    uv_snapshot!(context.filters(), context.python_install().arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
    "###);

    // Should not work with older Python versions
    uv_snapshot!(context.filters(), context.python_install().arg("3.12t"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: No download found for request: cpython-3.12t-[PLATFORM]
    "###);

    uv_snapshot!(context.filters(), context.python_uninstall().arg("--all"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Searching for Python installations
    Uninstalled 2 versions in [TIME]
     - cpython-3.13.0-[PLATFORM]
     - cpython-3.13.0+freethreaded-[PLATFORM]
    "###);
}

#[test]
fn python_install_invalid_request() {
    let context: TestContext = TestContext::new_with_versions(&[]).with_filtered_python_keys();

    // Request something that is not a Python version
    uv_snapshot!(context.filters(), context.python_install().arg("foobar"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: Cannot download managed Python for request: executable name `foobar`
    "###);

    // Request a version we don't have a download for
    uv_snapshot!(context.filters(), context.python_install().arg("3.8.0"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: No download found for request: cpython-3.8.0-[PLATFORM]
    "###);

    // Request a version we don't have a download for mixed with one we do
    uv_snapshot!(context.filters(), context.python_install().arg("3.8.0").arg("3.12"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: No download found for request: cpython-3.8.0-[PLATFORM]
    "###);
}

#[test]
fn python_install_default() {
    let context: TestContext = TestContext::new_with_versions(&[]).with_filtered_python_keys();

    let bin_python_minor = context
        .temp_dir
        .child("bin")
        .child(format!("python3.13{}", std::env::consts::EXE_SUFFIX));

    let bin_python_major = context
        .temp_dir
        .child("bin")
        .child(format!("python3{}", std::env::consts::EXE_SUFFIX));

    let bin_python_default = context
        .temp_dir
        .child("bin")
        .child(format!("python{}", std::env::consts::EXE_SUFFIX));

    // `--preview` is required for `--default`
    uv_snapshot!(context.filters(), context.python_install().arg("--default"), @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    The `--default` flag is only available in preview mode; add the `--preview` flag to use `--default.
    "###);

    // Install a specific version
    uv_snapshot!(context.filters(), context.python_install().arg("--preview").arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
       + python3.13
    "###);

    // Only the minor versioned executable should be installed
    bin_python_minor.assert(predicate::path::exists());
    bin_python_major.assert(predicate::path::missing());
    bin_python_default.assert(predicate::path::missing());

    // Install again, with `--default`
    uv_snapshot!(context.filters(), context.python_install().arg("--preview").arg("--default").arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
       + python
       + python3
    "###);

    // Now all the executables should be installed
    bin_python_minor.assert(predicate::path::exists());
    bin_python_major.assert(predicate::path::exists());
    bin_python_default.assert(predicate::path::exists());

    uv_snapshot!(context.filters(), context.python_uninstall().arg("--all"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Searching for Python installations
    Uninstalled Python 3.13.0 in [TIME]
     - cpython-3.13.0-[PLATFORM]
    "###);

    // The executables should be removed
    bin_python_minor.assert(predicate::path::missing());
    bin_python_major.assert(predicate::path::missing());
    bin_python_default.assert(predicate::path::missing());

    // Install the latest version
    uv_snapshot!(context.filters(), context.python_install().arg("--preview"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.13.0 in [TIME]
     + cpython-3.13.0-[PLATFORM]
       + python
       + python3
       + python3.13
    "###);

    // Since it's a bare install, we should include all of the executables
    bin_python_minor.assert(predicate::path::exists());
    bin_python_major.assert(predicate::path::exists());
    bin_python_default.assert(predicate::path::exists());

    uv_snapshot!(context.filters(), context.python_uninstall().arg("3.13"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Searching for Python versions matching: Python 3.13
    Uninstalled Python 3.13.0 in [TIME]
     - cpython-3.13.0-[PLATFORM]
    "###);

    // We should remove all the executables
    bin_python_minor.assert(predicate::path::missing());
    bin_python_major.assert(predicate::path::missing());
    bin_python_default.assert(predicate::path::missing());

    // Install multiple versions
    uv_snapshot!(context.filters(), context.python_install().arg("--preview").arg("3.12").arg("3.13").arg("--default"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed 2 versions in [TIME]
     + cpython-3.12.7-[PLATFORM]
       + python
       + python3
       + python3.12
     + cpython-3.13.0-[PLATFORM]
       + python3.13
    "###);

    bin_python_minor.assert(predicate::path::exists());
    bin_python_major.assert(predicate::path::exists());
    bin_python_default.assert(predicate::path::exists());

    let bin_python_minor_12 = context
        .temp_dir
        .child("bin")
        .child(format!("python3.12{}", std::env::consts::EXE_SUFFIX));

    bin_python_minor_12.assert(predicate::path::exists());
    assert!(is_same_file(&bin_python_minor_12, &bin_python_major).unwrap());
    assert!(is_same_file(&bin_python_minor_12, &bin_python_default).unwrap());
}
