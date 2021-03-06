use delivery::git::git_command;
use delivery::utils::copy_recursive;
use delivery::utils::say;
use std::io::prelude::*;
use tempdir::TempDir;
use std::fs::File;
use support::paths::fixture_file;
use std::process::Command;
use std::env;
use rustc_serialize::json::Json;
use delivery::utils::path_join_many::PathJoinMany;

// ** Functions used in tests **

fn setup() {
    say::turn_off_spinner();
}

/// Sets up a mock delivery git project from the test_repo fixture.
/// Includes copying in the `.delivery/config.json` that you plan
/// on using.
fn setup_mock_delivery_project_git(dot_config: &str) -> TempDir {
    let tmpdir = TempDir::new("mock-delivery-remote").unwrap();
    let test_repo_path = fixture_file("test_repo");
    panic_on_error!(copy_recursive(&test_repo_path.join(".delivery"), &tmpdir.path().to_path_buf()));
    panic_on_error!(copy_recursive(&test_repo_path.join("README.md"), &tmpdir.path().to_path_buf()));
    panic_on_error!(copy_recursive(&test_repo_path.join("cookbooks"), &tmpdir.path().to_path_buf()));
    panic_on_error!(copy_recursive(&fixture_file(dot_config), &tmpdir.path().join_many(&[".delivery", "config.json"])));
    panic_on_error!(git_command(&["init", tmpdir.path().to_str().unwrap()], tmpdir.path()));
    panic_on_error!(git_command(&["add", "."], tmpdir.path()));
    panic_on_error!(git_command(&["commit", "-a", "-m", "Initial Commit"], tmpdir.path()));
    tmpdir
}

/// Given a path, it copies the build cookbook into it, and turns it
/// into a git repository
fn setup_build_cookbook_project(tmpdir: &TempDir) {
    let build_cookbook_path = fixture_file("delivery_test");
    panic_on_error!(copy_recursive(&build_cookbook_path, &tmpdir.path().to_path_buf()));
    panic_on_error!(git_command(&["init", tmpdir.path().join("delivery_test").to_str().unwrap()], &tmpdir.path().join("delivery_test")));
    panic_on_error!(git_command(&["add", "."], &tmpdir.path().join("delivery_test")));
    panic_on_error!(git_command(&["commit", "-a", "-m", "Initial Commit"], &tmpdir.path().join("delivery_test")));
}

/// Clones a mock delivery git project to a local copy, as if it was
/// on a workstation. Also creates a mock delivery remote pointing at
/// the on-disk mocked delivery project.
fn setup_local_project_clone(delivery_project_git: &TempDir) -> TempDir {
    let tmpdir = TempDir::new("local-project").unwrap();
    panic_on_error!(git_command(&["clone",
                                  delivery_project_git.path().to_str().unwrap(),
                                  tmpdir.path().to_str().unwrap()
                                 ], tmpdir.path()));
    panic_on_error!(git_command(&["remote", "add", "delivery", delivery_project_git.path().to_str().unwrap()], tmpdir.path()));
    let result = panic_on_error!(delivery_cmd()
                    .arg("setup")
                    .arg("--user").arg("cavalera")
                    .arg("--server").arg("localhost")
                    .arg("--ent").arg("family")
                    .arg("--org").arg("sepultura")
                    .arg("--for").arg("master")
                    .arg("--config-path").arg(tmpdir.path().to_str().unwrap())
                    .current_dir(tmpdir.path()).output());
    if ! result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("Failed 'delivery setup'\nOUT: {}\nERR: {}\nPath: {}", &output, &error, tmpdir.path().to_str().unwrap());
    }
    tmpdir
}

/// Makes a change to a project on the named branch. Creates a
/// file named `filename` and writes some stuff to it.
///
/// When it returns, the project you pass in will be left on your
/// new branch.
fn setup_change(tmpdir: &TempDir, branch: &str, filename: &str) {
    panic_on_error!(git_command(&["checkout", "master"], tmpdir.path()));
    panic_on_error!(git_command(&["branch", branch], tmpdir.path()));
    {
        let mut f = panic_on_error!(File::create(&tmpdir.path().join(filename)));
        panic_on_error!(f.write_all(b"I like cookies"));
    }
    panic_on_error!(git_command(&["add", "."], tmpdir.path()));
    panic_on_error!(git_command(&["commit", "-a", "-m", filename], tmpdir.path()));
}

/// Checks out the named branch
fn setup_checkout_branch(tmpdir: &TempDir, branch: &str) {
    panic_on_error!(git_command(&["checkout", branch], tmpdir.path()));
}

/// Calls delivery review, and creates the two stub branches that the
/// api would create (`_reviews/PIPELINE/BRANCH/1` and `_reviews/PIPELINE/BRANCH/latest`)
fn delivery_review(local: &TempDir, remote: &TempDir, branch: &str, pipeline: &str) {
    panic_on_error!(git_command(&["checkout", branch], local.path()));
    let result = panic_on_error!(delivery_cmd().arg("review").arg("--no-open").arg("--for").arg(pipeline).current_dir(&local.path()).output());
    if ! result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("Failed 'delivery review'\nOUT: {}\nERR: {}\nPath: {}", &output, &error, local.path().to_str().unwrap());
    }
    // Stub out the behavior of the delivery-api
    panic_on_error!(git_command(&["branch", &format!("_reviews/{}/{}/1", pipeline, branch)], remote.path()));
    panic_on_error!(git_command(&["branch", &format!("_reviews/{}/{}/latest", pipeline, branch)], remote.path()));
}

/// Returns a Command set to the delivery binary created when you
/// ran `cargo test`.
fn delivery_cmd() -> Command {
    let mut delivery_path = env::current_exe().unwrap();
    delivery_path.pop();
    Command::new(delivery_path.join("delivery").to_str().unwrap())
}

/// A handy debugging function. Insert it when you want to sleep,
/// pass it a tmpdir, and you can inspect it.
///
/// Make sure you run `cargo test -- --nocapture` to see the output.
#[allow(dead_code)]
fn debug_sleep(tmpdir: &TempDir) {
    println!("Sleeping for 1000 seconds for {:?}", tmpdir.path());
    panic_on_error!(Command::new("sleep").arg("1000").output());
}

// ** Actual tests **

// Tests `delivery review`. Fails if the command fails, or if we fail to create
// the remote branch _for/master/rust/test, which is what we need to push to
// the API server.
test!(review {
    let delivery_project_git = setup_mock_delivery_project_git("path_config.json");
    let local_project = setup_local_project_clone(&delivery_project_git);
    setup_change(&local_project, "rust/test", "freaky");
    delivery_review(&local_project, &delivery_project_git, "rust/test", "master");
    setup_checkout_branch(&delivery_project_git, "_for/master/rust/test");
});

test!(job_verify_unit_with_path_config {
    let delivery_project_git = setup_mock_delivery_project_git("path_config.json");
    let local_project = setup_local_project_clone(&delivery_project_git);
    let job_root = TempDir::new("job-root").unwrap();
    setup_change(&local_project, "rust/test", "freaky");
    let result = panic_on_error!(delivery_cmd().
                                 arg("job").
                                 arg("verify").
                                 arg("unit").
                                 arg("--no-spinner").
                                 arg("--job-root").arg(job_root.path().to_str().unwrap()).
                                 current_dir(local_project.path()).output());
    if ! result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("Failed 'delivery job verify unit'\nOUT: {}\nERR: {}\nPath: {}", &output, &error, local_project.path().to_str().unwrap());
    }
});

test!(job_verify_unit_with_git_config {
    let delivery_project_git = setup_mock_delivery_project_git("git_config.json");
    let local_project = setup_local_project_clone(&delivery_project_git);
    let job_root = TempDir::new("job-root").unwrap();
    setup_build_cookbook_project(&job_root);
    setup_change(&local_project, "rust/test", "freaky");
    let result = panic_on_error!(delivery_cmd().
                                 arg("job").
                                 arg("verify").
                                 arg("unit").
                                 arg("--no-spinner").
                                 arg("--job-root").arg(job_root.path().to_str().unwrap()).
                                 current_dir(local_project.path()).output());
    if ! result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("Failed 'delivery verify unit'\nOUT: {}\nERR: {}\nPath: {}", &output, &error, local_project.path().to_str().unwrap());
    }
});

test!(job_verify_unit_with_supermarket_config {
    let delivery_project_git = setup_mock_delivery_project_git("supermarket_config.json");
    let local_project = setup_local_project_clone(&delivery_project_git);
    let job_root = TempDir::new("job-root").unwrap();
    setup_change(&local_project, "rust/test", "freaky");
    let result = panic_on_error!(delivery_cmd().
                                 arg("job").
                                 arg("verify").
                                 arg("unit").
                                 arg("--no-spinner").
                                 arg("--job-root").arg(job_root.path().to_str().unwrap()).
                                 current_dir(local_project.path()).output());
    if result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("The 'delivery verify unit' ought to have failed\nOUT: {}\nERR: {}\nPath: {}", &output, &error, local_project.path().to_str().unwrap());
    }
    assert!(job_root.path().join_many(&["chef", "cookbooks", "httpd"]).is_dir());
    assert!(job_root.path().join_many(&["chef", "cookbooks", "httpd", "templates", "default", "magic.erb"]).is_file());
});

test!(job_verify_dna_json {
    let delivery_project_git = setup_mock_delivery_project_git("path_config.json");
    let local_project = setup_local_project_clone(&delivery_project_git);
    let job_root = TempDir::new("job-root").unwrap();
    setup_change(&local_project, "rust/test", "freaky");
    let result = panic_on_error!(delivery_cmd().
                                 arg("job").
                                 arg("verify").
                                 arg("unit").
                                 arg("--no-spinner").
                                 arg("--job-root").arg(job_root.path().to_str().unwrap()).
                                 current_dir(local_project.path()).output());
    if ! result.status.success() {
        let output = String::from_utf8_lossy(&result.stdout);
        let error = String::from_utf8_lossy(&result.stderr);
        panic!("Failed 'delivery job verify unit'\nOUT: {}\nERR: {}\nPath: {}", &output, &error, local_project.path().to_str().unwrap());
    }
    let mut dna_file = panic_on_error!(File::open(&job_root.path().join_many(&["chef", "dna.json"])));
    let mut dna_json = String::new();
    panic_on_error!(dna_file.read_to_string(&mut dna_json));
    let dna_data = panic_on_error!(Json::from_str(&dna_json));
    match dna_data.find_path(&["delivery", "workspace", "repo"]) {
        Some(data) => {
            assert!(data.is_string());
            assert_eq!(data.as_string().unwrap(), job_root.path().join("repo").to_str().unwrap());
        },
        None => panic!("No delivery/workspace/repo, {}", dna_data)
    };
    match dna_data.find_path(&["delivery", "workspace", "chef"]) {
        Some(data) => {
            assert!(data.is_string());
            assert_eq!(data.as_string().unwrap(), job_root.path().join("chef").to_str().unwrap());
        },
        None => panic!("No delivery/workspace/chef, {}", dna_data)
    };
    match dna_data.find_path(&["delivery", "workspace", "cache"]) {
        Some(data) => {
            assert!(data.is_string());
            assert_eq!(data.as_string().unwrap(), job_root.path().join("cache").to_str().unwrap());
        },
        None => panic!("No delivery/workspace/cache, {}", dna_data)
    };
    match dna_data.find_path(&["delivery", "workspace", "root"]) {
        Some(data) => {
            assert!(data.is_string());
            assert_eq!(data.as_string().unwrap(), job_root.path().to_str().unwrap());
        },
        None => panic!("No delivery/workspace/root, {}", dna_data)
    };
    match dna_data.find_path(&["delivery_builder", "build_user"]) {
        Some(data) => {
            assert!(data.is_string());
            assert_eq!(data.as_string().unwrap(), "dbuild");
        },
        None => panic!("No delivery_builderl/build_user, {}", dna_data)
    };
});

