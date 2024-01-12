use crate::config::Config;
use crate::{Download, Launch};

#[test]
fn test_os () {
    let os_type = std::env::consts::OS;
    println!("操作系统类型: {}", os_type);
    assert_eq!(os_type, "windows");
}

#[test]
fn test_download() {
    let cfg: Config = confy::load("rimca", "config").unwrap();

    let dl = Download {
        instance: "test".to_string(),
        version: Some(String::from("1.16.4")),
        forge: None,
        fabric: None,
    };
    rimca::download(&dl.instance, dl.version, Some(String::from("vanilla")), &cfg.base_dir).unwrap()
}

#[test]
fn test_launch() {
    let cfg: Config = confy::load("rimca", "config").unwrap();

    let launch = Launch {
        // 实例名
        instance: "test".to_string(),
        username: "Watson17".to_string(),
        game_output: true,
    };
    rimca::launch(&launch.instance, &launch.username, launch.game_output, &cfg.base_dir).unwrap()
}