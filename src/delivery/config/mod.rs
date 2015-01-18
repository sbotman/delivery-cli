#![allow(unstable)]
extern crate "rustc-serialize" as rustc_serialize;
extern crate toml;

pub use errors;
use errors::{DeliveryError, Kind};
use std::io;
use std::io::File;
use std::io::fs::{PathExtensions, mkdir_recursive};
use std::default::Default;
use utils::say::{say, sayln};
use rustc_serialize::Encodable;

#[derive(RustcEncodable, Clone)]
pub struct Config {
    pub server: Option<String>,
    pub user: Option<String>,
    pub enterprise: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub git_port: Option<String>,
    pub pipeline: Option<String>
}

impl Default for Config {
    fn default() -> Config {
        Config{
            server: None,
            enterprise: None,
            organization: None,
            project: None,
            user: None,
            git_port: Some(String::from_str("8989")),
            pipeline: Some(String::from_str("master"))
        }
    }
}

macro_rules! config_accessor_for {
    ($name:ident, $set_name:ident, $err_msg:expr) => (
        impl Config {
            pub fn $name(self) -> Result<String, DeliveryError> {
                match self.$name {
                    Some(v) => Ok(v.clone()),
                    None => Err(DeliveryError{ kind: Kind::MissingConfig, detail: Some(String::from_str($err_msg)) })
                }
            }

            pub fn $set_name(mut self, $name: &str) -> Config {
                if !$name.is_empty() {
                    self.$name = Some(String::from_str($name));
                }
                self
            }
        }
    )
}

config_accessor_for!(server, set_server, "Server not set; try --server");
config_accessor_for!(user, set_user, "User not set; try --user");
config_accessor_for!(enterprise, set_enterprise, "Enterprise not set; try --ent");
config_accessor_for!(organization, set_organization, "Organization not set; try --org");
config_accessor_for!(project, set_project, "Project not set; try --project");
config_accessor_for!(git_port, set_git_port, "Git Port not set");
config_accessor_for!(pipeline, set_pipeline, "Pipeline not set; try --for");

impl Config {
    pub fn load_config(cwd: &Path) -> Result<Config, DeliveryError> {
        let have_config = Config::have_dot_delivery_cli(cwd);
        match have_config.as_ref() {
            Some(path) => {
                let toml = try!(Config::read_file(path));
                match Config::parse_config(toml.as_slice()) {
                    Ok(c) => return Ok(c),
                    Err(_) => return Ok(Default::default())
                }
            },
            None => return Ok(Default::default())
        }
    }

    pub fn write_file(&self, path: &Path) -> Result<(), DeliveryError> {
        let write_dir = path.join_many(&[".delivery"]);
        if !write_dir.is_dir() {
            try!(mkdir_recursive(&write_dir, io::USER_RWX));
        }
        let write_path = path.join_many(&[".delivery", "cli.toml"]);
        say("white", "Writing configuration to ");
        sayln("yellow", format!("{}", write_path.display()).as_slice());
        let mut f = try!(File::create(&write_path));
        let toml_string = toml::encode_str(self);
        sayln("magenta", "New configuration");
        sayln("magenta", "-----------------");
        say("white", toml_string.as_slice());
        try!(f.write(toml_string.as_bytes()));
        Ok(())
    }

    pub fn parse_config(toml: &str) -> Result<Config, DeliveryError> {
        let mut parser = toml::Parser::new(toml);
        match parser.parse() {
            Some(value) => { return Config::set_values_from_toml_table(value); },
            None => {
                return Err(DeliveryError{
                    kind: Kind::ConfigParse,
                    detail: Some(format!("Parse errors: {:?}", parser.errors))
                });
            }
        }
    }

    fn set_values_from_toml_table(table: toml::Table) -> Result<Config, DeliveryError> {
        let mut config: Config = Default::default();
        config.server = Config::stringify_values(table.get("server"));
        config.project = Config::stringify_values(table.get("project"));
        config.enterprise = Config::stringify_values(table.get("enterprise"));
        config.organization = Config::stringify_values(table.get("organization"));
        config.user = Config::stringify_values(table.get("user"));
        config.git_port = Config::stringify_values(table.get("git_port"));
        return Ok(config);
    }

    fn read_file(path: &Path) -> Result<String, DeliveryError>  {
        let toml = try!(File::open(path).read_to_string());
        Ok(toml)
    }

    fn stringify_values(toml_value: Option<&toml::Value>) -> Option<String> {
        match toml_value {
            Some(value) => {
                let is_string = value.as_str();
                match is_string {
                    Some(vstr) => return Some(String::from_str(vstr)),
                    None => return None
                }
            },
            None => {
                return None;
            }
        }
    }

    fn check_dot_delivery_cli(path: Path) -> Option<Path> {
        let dot_git = path.join_many(&[".delivery", "cli.toml"]);
        debug!("Checking {}", dot_git.display());
        let is_file: Option<Path> = if dot_git.is_file() {
            Some(dot_git)
        } else {
            None
        };
        is_file
    }

    fn have_dot_delivery_cli(orig_path: &Path) -> Option<Path> {
        let mut path = orig_path.clone();
        loop {
            let check_result: Option<Path> = Config::check_dot_delivery_cli(path.clone());
            match check_result.as_ref() {
                Some(_) => { return check_result.clone() }
                None => {
                    if path.pop() { } else { return check_result.clone() }
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parse_config() {
        let toml = r#"
            server = "127.0.0.1"
            enterprise = "chef"
            organization = "chef"
            user = "adam"
"#;
        let config_result = Config::parse_config(toml);
        match config_result {
            Ok(config) => {
                assert_eq!(config.server, Some(String::from_str("127.0.0.1")));
                assert_eq!(config.git_port, None);
            },
            Err(e) => {
                panic!("Failed to parse: {:?}", e.detail)
            }
        }
    }
}
