use assert_cmd::Command;
use predicates::str::contains;

/// The binary, with the colour variables of the parent environment removed so
/// that text output is plain unless a test sets them.
fn tblctl() -> Command {
    let mut cmd = Command::cargo_bin("tblctl").unwrap();
    cmd.env_remove("CLICOLOR_FORCE").env_remove("NO_COLOR");
    cmd
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

#[test]
fn top_level_help_ends_with_the_readme_link() {
    let out = tblctl().arg("--help").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.trim_end()
            .ends_with("https://github.com/driftsys/tblctl#readme"),
        "{text}"
    );
}

#[test]
fn every_command_help_has_examples() {
    for cmd in ["classify", "import", "check", "export"] {
        tblctl()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(contains("Examples:"))
            .stdout(contains(format!("tblctl {cmd} ")));
    }
}

#[test]
fn positional_arguments_have_descriptions() {
    tblctl()
        .args(["check", "--help"])
        .assert()
        .stdout(contains("Markdown files to check"));
    tblctl()
        .args(["classify", "--help"])
        .assert()
        .stdout(contains(
            "Source files: xlsx, xlsm, xls, xlsb, ods, docx, csv, md",
        ));
    tblctl()
        .args(["import", "--help"])
        .assert()
        .stdout(contains(
            "Source file: xlsx, xlsm, xls, xlsb, ods, docx, csv, md",
        ));
    tblctl()
        .args(["export", "--help"])
        .assert()
        .stdout(contains("Markdown or CSV file to export"));
}
