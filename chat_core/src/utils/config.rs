use anyhow::{bail, Result};
use serde::Deserialize;
use std::env;
use std::fs::File;

pub fn load_config<T: for<'de> Deserialize<'de>>(env_name: &str, filename: &str) -> Result<T> {
    // read from  ./app.yml, or /etc/config/app.yml, or from env CHAT_CONFIG
    let ret = match (
        File::open(filename),
        File::open(format!("/etc/config/{filename}")),
        env::var(env_name),
    ) {
        (Ok(reader), _, _) => serde_yaml::from_reader(reader),
        (_, Ok(reader), _) => serde_yaml::from_reader(reader),
        (_, _, Ok(path)) => serde_yaml::from_reader(File::open(path)?),
        _ => bail!("Config file not found"),
    };
    Ok(ret?)
}
