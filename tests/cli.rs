use assert_cmd::Command;
use predicates::str::contains;

fn tblctl() -> Command {
    Command::cargo_bin("tblctl").unwrap()
}

#[test]
fn version_prints_crate_version() {
    tblctl()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_all_subcommands() {
    tblctl()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("classify"))
        .stdout(contains("import"))
        .stdout(contains("check"))
        .stdout(contains("export"));
}
