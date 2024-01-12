pub mod api;

pub mod models;

pub use models::{Meta, Assets};

use crate::{Instance, Paths};
use crate::download::DownloadSequence;
use crate::launch::LaunchSequence;
use crate::error::{LaunchError, LaunchArguments, DownloadError, StateError};
use crate::state::Component;
use crate::verify::is_file_valid;
use crate::vanilla::api::Version;

use std::io::BufReader;
use std::path::PathBuf;
use nizziel::{Download, Downloads};
use crate::vanilla::models::Library;

pub struct Vanilla {
    pub version: Version,
    pub meta: Meta,
}

impl Vanilla {
    pub fn new(paths: &Paths, version: Option<String>) -> Result<Self, DownloadError> {
        let version = match &version {
            Some(ver) => {
                api::versions(true)?.into_iter().find(|v| v.id.eq(ver))
                    .ok_or_else(|| DownloadError::GameVersionNotFound(ver.to_string()))?
            }

            None => api::latest(false)?
        };

        let meta = {
            let path = paths.get("meta")?.join("net.minecraft").join(format!("{}.json", &version.id));
            if let Ok(file) = std::fs::File::open(&path) {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader)?
            } else {
                let meta_str = nizziel::blocking::download(&version.url, &path, false)?;
                serde_json::from_slice::<Meta>(&meta_str)?
            }
        };

        Ok(Self {
            version,
            meta,
        })
    }
}

fn process_natives(key_option: Option<&String>, natives_dir: PathBuf, lib: &Library, dls: &mut Downloads) -> Result<(), DownloadError> {
    if let Some(key) = key_option {
        if let Some(url) = lib.downloads.classifiers.as_ref().ok_or_else(|| DownloadError::LibraryNoClassifiers(lib.name.clone()))?.get(key) {
            dls.downloads.push(Download {
                url: url.url.to_string(),
                path: natives_dir.clone(),
                unzip: true,
            });
        }
    }
    Ok(())
}

impl DownloadSequence for Instance<Vanilla> {
    fn collect_urls(&mut self) -> Result<Downloads, DownloadError> {
        let mut dls = Downloads { retries: 5, ..Default::default() };
        let meta = &self.inner.meta;

        let path = self.paths.get("libraries")?
            .join("com")
            .join("mojang")
            .join("minecraft")
            .join(&self.inner.version.id)
            .join(format!("minecraft-{}-client.jar", self.inner.version.id));

        if !path.exists() || !is_file_valid(&path, &meta.downloads.client.sha1)? {
            dls.downloads.push(Download {
                url: meta.downloads.client.url.clone(),
                path,
                unzip: false,
            });
        }

        let natives_dir = self.paths.get("natives")?;

        // TODO 根据当前的系统类型选择正确的nav文件解压
        let os_type = std::env::consts::OS;

        for lib in &meta.libraries {
            // libraries
            if let Some(artifact) = &lib.downloads.artifact {
                let path = self.paths.get("libraries")?.join(&artifact.path);
                if !path.exists() || !is_file_valid(&path, &artifact.sha1)? {
                    dls.downloads.push(Download {
                        url: artifact.url.clone(),
                        path,
                        unzip: false,
                    });
                }
            }

            match os_type {
                "windows" => process_natives(lib.natives.as_ref().and_then(|n| n.windows.as_ref()), natives_dir.clone(), lib, &mut dls)?,
                "linux" => process_natives(lib.natives.as_ref().and_then(|n| n.linux.as_ref()), natives_dir.clone(), lib, &mut dls)?,
                "macos" => process_natives(lib.natives.as_ref().and_then(|n| n.macos.as_ref()), natives_dir.clone(), lib, &mut dls)?,
                _ => {} // 或者处理不支持的操作系统类型
            }
        }

        // assets
        let asset_id = &meta.asset_index.id;
        let url = &meta.asset_index.url;
        let path = self.paths.get("assets")?.join("indexes").join(format!("{}.json", asset_id));

        let assets_str = nizziel::blocking::download(url, &path, false)?;
        let assets: Assets = serde_json::from_slice(&assets_str)?;

        if asset_id.eq("pre-1.6") || asset_id.eq("legacy") {
            for (key, hash) in assets.objects {
                let hash_head = &hash.hash[0..2];
                let path = self.paths.get("instance")?.join("resources").join(key);

                if !path.exists() && is_file_valid(&path, &hash.hash)? {
                    dls.downloads.push(Download {
                        url: format!("https://resources.download.minecraft.net/{}/{}", hash_head, hash.hash),
                        path,
                        unzip: false,
                    });
                }
            }
        } else {
            let objects_dir = self.paths.get("assets")?.join("objects");
            for hash in assets.objects.values() {
                let hash_head = &hash.hash[0..2];
                let path = objects_dir.join(hash_head).join(&hash.hash);

                if !path.exists() {
                    dls.downloads.push(Download {
                        url: format!("https://resources.download.minecraft.net/{}/{}", hash_head, hash.hash),
                        path,
                        unzip: false,
                    });
                }
            }
        }

        Ok(dls)
    }

    fn create_state(&mut self) -> Result<(), DownloadError> {
        self.state.components.insert(
            "java".to_string(),
            Component::JavaComponent {
                path: "java".to_string(),
                arguments: None,
            },
        );

        self.state.components.insert(
            "net.minecraft".to_string(),
            Component::GameComponent {
                version: self.inner.version.id.to_string()
            },
        );

        Ok(())
    }
}

impl LaunchSequence for Instance<Vanilla> {
    fn get_main_class(&self) -> Result<String, LaunchError> {
        Ok(self.inner.meta.main_class.clone())
    }

    fn get_game_options(&self, username: &str) -> Result<Vec<String>, LaunchError> {
        let meta = &self.inner.meta;

        if let Component::GameComponent { version } = self.state.get_component("net.minecraft")? {
            let asset_index = &self.inner.meta.asset_index.id;
            let game_assets = self.paths.get("resources")?;
            let assets_path = self.paths.get("assets")?;

            let arguments = meta.arguments.get("game").ok_or(LaunchError::ArgumentsNotFound(LaunchArguments::Game))?;
            let account = crate::auth::Accounts::get(self.paths.get("accounts")?)?.get_account(username).unwrap_or(crate::auth::Account::default());

            return Ok(arguments.iter().map(|x| x
                .replace("${auth_player_name}", username)
                .replace("${version_name}", version)
                .replace("${game_directory}", ".")
                .replace("${assets_root}", assets_path.to_str().unwrap())
                .replace("${assets_index_name}", asset_index)
                .replace("${auth_uuid}", &account.uuid)
                .replace("${auth_access_token}", &account.access_token)
                .replace("${user_type}", "mojang")
                .replace("${version_type}", &meta.r#type)
                .replace("${user_properties}", "{}")
                // .replace("${resolution_width}", "1920")
                // .replace("${resolution_height}", "1080")
                .replace("${game_assets}", game_assets.to_str().unwrap())
                .replace("${auth_session}", "{}")
            ).collect());
        }

        Err(LaunchError::StateError(StateError::ComponentNotFound(String::from("net.minecraft"))))
    }

    fn get_classpath(&self) -> Result<String, LaunchError> {
        let meta = &self.inner.meta;
        let libraries = self.paths.get("libraries")?;

        let mut classpath = String::with_capacity((libraries.to_str().unwrap().len() * meta.libraries.len())
            + (meta.libraries.len() * 2)
            + meta.libraries.iter().map(|lib| lib.downloads.artifact.as_ref().map_or(0, |a| a.path.len())).sum::<usize>()
        );

        'outer: for lib in &meta.libraries {
            if let Some(rules) = &lib.rules {
                for rule in rules {
                    if let Some(os) = &rule.os {
                        if let Some(name) = &os.name {
                            if rule.action.eq("allow") && name.ne("linux") ||
                                rule.action.eq("disallow") && name.eq("linux") {
                                continue 'outer;
                            }
                        }
                    }
                }
            }

            if let Some(artifact) = &lib.downloads.artifact {
                classpath.push_str(libraries.to_str().unwrap());
                classpath.push('/');
                classpath.push_str(&artifact.path);
                classpath.push(';');
            }
        }

        let jar_name = format!("minecraft-{}-client.jar", meta.id);
        let jar_path = libraries.join("com").join("mojang").join("minecraft").join(meta.id.clone()).join(jar_name);
        classpath.push_str(jar_path.to_str().unwrap());
        Ok(classpath)
    }

    fn get_jvm_arguments(&self, classpath: &str) -> Result<Vec<String>, LaunchError> {
        let natives_directory = self.paths.get("natives")?;

        let mut jvm_arguments = {
            if let Some(arguments) = &self.inner.meta.arguments.get("jvm") {
                arguments.iter().map(|x| x
                    .replace("${natives_directory}", natives_directory.to_str().unwrap())
                    .replace("${launcher_name}", "rimca")
                    .replace("${launcher_version}", "3.0")
                    .replace("${classpath}", classpath)
                ).collect()
            } else {
                let mut jvm_arguments = Vec::with_capacity(3 + classpath.len());
                jvm_arguments.push(format!("-Djava.library.path={}", &natives_directory.to_str().unwrap()));
                jvm_arguments.push("-cp".to_string());
                jvm_arguments.push(classpath.to_string());
                jvm_arguments
            }
        };

        if let Ok(Component::JavaComponent { arguments, .. }) = &self.state.get_component("java") {
            if let Some(args) = arguments {
                jvm_arguments.extend(args.split_whitespace().map(|s| s.to_string()));
            }

            return Ok(jvm_arguments);
        }

        Err(LaunchError::StateError(StateError::ComponentNotFound(String::from("java"))))
    }
}