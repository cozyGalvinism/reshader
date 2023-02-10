use std::{
    fmt::{Display, Formatter},
    io::{Read, Seek},
    path::{Path, PathBuf},
    process::exit,
};

use clap::Parser;
use config::Config;
use dircpy::CopyBuilder;
use inquire::error::InquireResult;
use strum::{EnumIter, IntoEnumIterator};

use crate::prelude::*;

mod cli;
mod config;
mod prelude;
mod tui;

static QUALIFIER: &str = "eu";
static ORGANIZATION: &str = "cozysoft";
static APPLICATION: &str = "reshader";

static APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, EnumIter)]
enum InstallOption {
    ReShade,
    ReShadeVanilla,
    Presets,
    Uninstall,
    Quit,
}

impl Display for InstallOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallOption::ReShade => write!(
                f,
                "Install/Update ReShade (with addon support, recommended)"
            ),
            InstallOption::ReShadeVanilla => write!(f, "Install/Update ReShade (vanilla)"),
            InstallOption::Presets => write!(
                f,
                "Install/Update GShade shaders and presets (install ReShade first)"
            ),
            InstallOption::Uninstall => write!(f, "Uninstall ReShade/GShade"),
            InstallOption::Quit => write!(f, "Quit"),
        }
    }
}

async fn download_file(client: &reqwest::Client, url: &str, path: &str) -> InquireResult<()> {
    let resp = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("reshader/{APP_VERSION}"),
        )
        .send()
        .await
        .map_err(|e| ReShaderError::Download(url.to_string(), e.to_string()))?
        .bytes()
        .await
        .map_err(|e| ReShaderError::Download(url.to_string(), e.to_string()))?;
    let mut out = tokio::fs::File::create(path).await?;
    let mut reader = tokio::io::BufReader::new(resp.as_ref());
    tokio::io::copy(&mut reader, &mut out).await?;
    Ok(())
}

async fn get_latest_reshade_version(
    client: &reqwest::Client,
    version: Option<String>,
    vanilla: bool,
) -> InquireResult<String> {
    let version = if let Some(version) = version {
        version
    } else {
        let tags = client
            .get("https://api.github.com/repos/crosire/reshade/tags")
            .header(
                reqwest::header::USER_AGENT,
                format!("reshader/{APP_VERSION}"),
            )
            .send()
            .await
            .map_err(|_| {
                ReShaderError::FetchLatestVersion("error while fetching tags".to_string())
            })?
            .json::<Vec<serde_json::Value>>()
            .await
            .map_err(|_| {
                ReShaderError::FetchLatestVersion("invalid json returned by github".to_string())
            })?;
        let mut tags = tags
            .iter()
            .map(|tag| tag["name"].as_str().unwrap().trim_start_matches('v'))
            .collect::<Vec<_>>();
        tags.sort_by(|a, b| {
            let a = semver::Version::parse(a).unwrap();
            let b = semver::Version::parse(b).unwrap();
            a.cmp(&b)
        });
        let latest = tags
            .last()
            .ok_or(ReShaderError::FetchLatestVersion(
                "no tags available".to_string(),
            ))?
            .trim_start_matches('v');

        latest.to_string()
    };

    // we're going to ignore that serving content over http in 2023 is terrible
    // just get a letsencrypt cert already
    if vanilla {
        Ok(format!(
            "http://static.reshade.me/downloads/ReShade_Setup_{version}.exe"
        ))
    } else {
        Ok(format!(
            "http://static.reshade.me/downloads/ReShade_Setup_{version}_Addon.exe"
        ))
    }
}

async fn download_reshade(
    client: &reqwest::Client,
    directory: &Path,
    vanilla: bool,
    version: Option<String>,
    specific_installer: &Option<String>,
) -> InquireResult<()> {
    let tmp = tempdir::TempDir::new("reshader_downloads")?;

    let reshade_path = if let Some(specific_installer) = specific_installer {
        PathBuf::from(specific_installer)
    } else {
        let reshade_url = get_latest_reshade_version(client, version, vanilla)
            .await
            .expect("Could not get latest ReShade version");
        let reshade_path = tmp.path().join("reshade.exe");

        download_file(client, &reshade_url, reshade_path.to_str().unwrap()).await?;
        reshade_path
    };

    let d3dcompiler_path = tmp.path().join("d3dcompiler_47.dll");
    download_file(
        client,
        "https://lutris.net/files/tools/dll/d3dcompiler_47.dll",
        d3dcompiler_path.to_str().unwrap(),
    )
    .await?;

    let exe = std::fs::File::open(&reshade_path).expect("Could not open ReShade installer");
    let mut exe = std::io::BufReader::new(exe);
    let mut buf = [0u8; 4];
    let mut offset = 0;
    // after 0x50, 0x4b, 0x03, 0x04, the zip archive starts
    loop {
        exe.read_exact(&mut buf)?;
        if buf == [0x50, 0x4b, 0x03, 0x04] {
            break;
        }
        offset += 1;
        exe.seek(std::io::SeekFrom::Start(offset))?;
    }
    let mut contents = zip::ZipArchive::new(exe).map_err(|_| ReShaderError::NoZipFile)?;

    let mut buf = Vec::new();
    contents
        .by_name("ReShade64.dll")
        .map_err(|_| ReShaderError::NoReShade64Dll)?
        .read_to_end(&mut buf)?;
    let reshade_dll = if vanilla {
        directory.join("ReShade64.Vanilla.dll")
    } else {
        directory.join("ReShade64.Addon.dll")
    };
    std::fs::write(reshade_dll, buf)?;

    std::fs::copy(d3dcompiler_path, directory.join("d3dcompiler_47.dll"))?;

    Ok(())
}

async fn install_reshade(
    config: &mut Config,
    data_dir: &Path,
    game_path: &Path,
    vanilla: bool,
) -> InquireResult<()> {
    if game_path.join("dxgi.dll").exists() {
        std::fs::remove_file(game_path.join("dxgi.dll"))?;
    }

    if game_path.join("d3dcompiler_47.dll").exists() {
        std::fs::remove_file(game_path.join("d3dcompiler_47.dll"))?;
    }

    if vanilla {
        std::os::unix::fs::symlink(
            data_dir.join("ReShade64.Vanilla.dll"),
            game_path.join("dxgi.dll"),
        )?;
    } else {
        std::os::unix::fs::symlink(
            data_dir.join("ReShade64.Addon.dll"),
            game_path.join("dxgi.dll"),
        )?;
    }
    std::os::unix::fs::symlink(
        data_dir.join("d3dcompiler_47.dll"),
        game_path.join("d3dcompiler_47.dll"),
    )?;

    tui::print_reshade_success();

    if config
        .game_paths
        .contains(&game_path.to_str().unwrap().to_string())
    {
        return Ok(());
    }

    config
        .game_paths
        .push(game_path.to_str().unwrap().to_string());

    Ok(())
}

async fn install_presets(
    directory: &PathBuf,
    presets_path: &PathBuf,
    shaders_path: &PathBuf,
) -> InquireResult<()> {
    let file = std::fs::File::open(presets_path)?;
    let mut presets_zip =
        zip::read::ZipArchive::new(file).map_err(|_| ReShaderError::ReadZipFile)?;
    presets_zip
        .extract(directory)
        .map_err(|_| ReShaderError::ExtractZipFile)?;

    CopyBuilder::new(
        directory.join("GShade-Presets-master"),
        directory.join("reshade-presets"),
    )
    .overwrite(true)
    .run()?;
    std::fs::remove_dir_all(directory.join("GShade-Presets-master"))?;

    let file = std::fs::File::open(shaders_path).expect("unable to open shaders file");
    let mut shaders_zip =
        zip::read::ZipArchive::new(file).map_err(|_| ReShaderError::ReadZipFile)?;
    shaders_zip
        .extract(directory)
        .map_err(|_| ReShaderError::ExtractZipFile)?;

    CopyBuilder::new(
        directory.join("gshade-shaders"),
        directory.join("reshade-shaders"),
    )
    .overwrite(true)
    .run()?;
    std::fs::remove_dir_all(directory.join("gshade-shaders"))?;

    Ok(())
}

fn uninstall(config: &mut Config, game_path: &Path) -> InquireResult<()> {
    let dxgi_path = PathBuf::from(&game_path).join("dxgi.dll");
    let d3dcompiler_path = PathBuf::from(&game_path).join("d3dcompiler_47.dll");
    let presets_path = PathBuf::from(&game_path).join("reshade-presets");
    let shaders_path = PathBuf::from(&game_path).join("reshade-shaders");

    if dxgi_path.exists() {
        std::fs::remove_file(dxgi_path)?;
    }
    if d3dcompiler_path.exists() {
        std::fs::remove_file(d3dcompiler_path)?;
    }
    if presets_path.exists() {
        std::fs::remove_dir_all(presets_path)?;
    }
    if shaders_path.exists() {
        std::fs::remove_dir_all(shaders_path)?;
    }

    config
        .game_paths
        .retain(|path| path != &game_path.to_str().unwrap().to_string());

    Ok(())
}

fn install_preset_for_game(data_dir: &Path, game_path: &Path) -> InquireResult<()> {
    let target_preset_path = PathBuf::from(game_path).join("reshade-presets");
    let target_shaders_path = PathBuf::from(game_path).join("reshade-shaders");

    if std::fs::read_link(target_preset_path).is_ok()
        || std::fs::read_link(target_shaders_path).is_ok()
    {
        return Ok(());
    }

    std::os::unix::fs::symlink(
        data_dir.join("reshade-presets"),
        PathBuf::from(game_path).join("reshade-presets"),
    )?;
    std::os::unix::fs::symlink(
        data_dir.join("reshade-shaders"),
        PathBuf::from(game_path).join("reshade-shaders"),
    )?;
    Ok(())
}

async fn tui(
    config: &mut Config,
    client: &reqwest::Client,
    data_dir: &PathBuf,
    config_path: &PathBuf,
    specific_installer: Option<String>,
) -> InquireResult<()> {
    loop {
        let install_option =
            inquire::Select::new("Select an option", InstallOption::iter().collect()).prompt()?;

        let result = match install_option {
            InstallOption::ReShade => {
                download_reshade(client, data_dir, false, None, &specific_installer).await?;
                let install_now = tui::prompt_install()?;
                if install_now {
                    let game_path = tui::prompt_game_path()?;
                    install_reshade(config, data_dir, &game_path, false).await
                } else {
                    Ok(())
                }
            }
            InstallOption::ReShadeVanilla => {
                download_reshade(client, data_dir, true, None, &specific_installer).await?;
                let install_now = tui::prompt_install()?;
                if install_now {
                    let game_path = tui::prompt_game_path()?;
                    install_reshade(config, data_dir, &game_path, true).await
                } else {
                    Ok(())
                }
            }
            InstallOption::Presets => {
                tui::print_gshade_warning();

                let open = tui::prompt_open_links()?;

                if open {
                    tui::print_gshade_file_move(data_dir);

                    let _ = open::that("https://github.com/HereInPlainSight/gshade_installer/blob/master/gshade_installer.sh#L352");
                    tui::print_gshade_hint();
                }

                let done = tui::prompt_confirm_move()?;

                if !done {
                    continue;
                }
                install_presets(
                    data_dir,
                    &data_dir.join("presets.zip"),
                    &data_dir.join("shaders.zip"),
                )
                .await?;

                if config.game_paths.is_empty() {
                    tui::print_presets_success_no_games(data_dir);
                    continue;
                }

                let install_for_games = tui::prompt_install_presets_for_games()?;

                if !install_for_games {
                    tui::print_presets_success_no_games(data_dir);
                    continue;
                }

                let game_paths = tui::prompt_select_game_paths(config.game_paths.clone())?;
                for game_path in &game_paths {
                    let game_path = PathBuf::from(game_path);
                    install_preset_for_game(data_dir, &game_path)?;
                }

                tui::print_presets_success();

                Ok(())
            }
            InstallOption::Uninstall => {
                if config.game_paths.is_empty() {
                    tui::print_no_game_paths();
                    return Ok(());
                }

                let game_path = tui::prompt_select_game_path_uninstall(config.game_paths.clone())?;
                uninstall(config, &game_path)?;

                Ok(())
            }
            InstallOption::Quit => break,
        };
        if let Err(e) = result {
            tui::print_error(e);
            continue;
        }

        let config_str =
            toml::to_string(&config).expect("if you see this error, the toml library is broken");
        std::fs::write(config_path, config_str)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> InquireResult<()> {
    if !cfg!(target_os = "linux") {
        println!("This installer is only supported on Linux");
        exit(1);
    }

    let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION);
    if dirs.is_none() {
        tui::print_no_home_dir();
        exit(1);
    }
    let dirs = dirs.unwrap();

    std::fs::create_dir_all(dirs.config_dir())?;
    std::fs::create_dir_all(dirs.data_dir())?;

    let config_dir = dirs.config_dir().to_path_buf();
    let data_dir = dirs.data_dir().to_path_buf();

    let config_path = config_dir.join("config.toml");
    let mut config = if config_path.exists() {
        let config_str = std::fs::read_to_string(&config_path)?;
        let result = toml::from_str(&config_str);
        if result.is_err() {
            tui::print_config_deserialization_error();
            exit(1);
        }
        result.unwrap()
    } else {
        let config = config::Config::default();
        let config_str =
            toml::to_string(&config).expect("if you see this error, the toml library is broken");
        std::fs::write(&config_path, config_str)?;
        config
    };
    let client = reqwest::Client::new();

    let args = cli::CliArgs::parse();

    let specific_installer = args.use_installer;

    if let Some(subcommand) = args.subcommand {
        match subcommand {
            cli::SubCommand::InstallReshade {
                vanilla,
                version,
                game,
            } => {
                download_reshade(&client, &data_dir, vanilla, version, &specific_installer).await?;
                if let Some(game) = game {
                    let game_path = PathBuf::from(game);
                    install_reshade(&mut config, &data_dir, &game_path, vanilla).await?;
                } else {
                    tui::print_reshade_success_no_games(&data_dir);
                }
            }
            cli::SubCommand::InstallPresets {
                all,
                game,
                presets,
                shaders,
            } => {
                let presets_path = PathBuf::from(presets);
                let shaders_path = PathBuf::from(shaders);

                install_presets(&data_dir, &presets_path, &shaders_path).await?;
                if all {
                    for game_path in &config.game_paths {
                        let game_path = PathBuf::from(game_path);
                        install_preset_for_game(&data_dir, &game_path)?;
                    }

                    tui::print_presets_success();
                } else if let Some(game) = game {
                    let game_path = PathBuf::from(game);
                    install_preset_for_game(&data_dir, &game_path)?;

                    tui::print_presets_success();
                }
            }
            cli::SubCommand::Uninstall { game } => {
                let game_path = PathBuf::from(game);
                uninstall(&mut config, &game_path)?;
            }
        }

        let config_str =
            toml::to_string(&config).expect("if you see this error, the toml library is broken");
        std::fs::write(config_path, config_str)?;
    } else {
        tui(
            &mut config,
            &client,
            &data_dir,
            &config_dir,
            specific_installer,
        )
        .await?;
    }

    Ok(())
}
