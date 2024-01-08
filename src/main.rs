use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
    process::exit,
};

use clap::Parser;
use cli::SubCommand;
use inquire::error::InquireResult;
use strum::{EnumIter, IntoEnumIterator};

use crate::config::Config;
use reshaderlib::{
    download_minimal_reshade_shaders, download_reshade, download_shader_collections,
    install_preset_for_game, install_presets, install_reshade, install_reshade_shaders, uninstall,
    SHADER_COLLECTIONS,
};

mod cli;
mod config;
mod tui;

static QUALIFIER: &str = "eu";
static ORGANIZATION: &str = "cozysoft";
static APPLICATION: &str = "reshader";

#[derive(Debug, EnumIter)]
enum InstallOption {
    ReShade,
    ReShadeVanilla,
    ReShadeShaders,
    GShadePresets,
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
            InstallOption::ReShadeShaders => write!(f, "Install/Update ReShade shaders"),
            InstallOption::GShadePresets => write!(
                f,
                "Install/Update GShade shaders and presets (install ReShade first)"
            ),
            InstallOption::Uninstall => write!(f, "Uninstall ReShade/GShade"),
            InstallOption::Quit => write!(f, "Quit"),
        }
    }
}

#[derive(Debug, EnumIter)]
enum ReShadeShadersOptions {
    Minimal,
    Select,
}

impl Display for ReShadeShadersOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReShadeShadersOptions::Minimal => write!(f, "Install minimal shaders"),
            ReShadeShadersOptions::Select => write!(f, "Select shaders to install"),
        }
    }
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
                    install_reshade(data_dir, &game_path, false).await?;
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
                } else {
                    Ok(())
                }
            }
            InstallOption::ReShadeVanilla => {
                download_reshade(client, data_dir, true, None, &specific_installer).await?;
                let install_now = tui::prompt_install()?;
                if install_now {
                    let game_path = tui::prompt_game_path()?;
                    install_reshade(data_dir, &game_path, true).await?;
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
                } else {
                    Ok(())
                }
            }
            InstallOption::ReShadeShaders => {
                let shader_install_option = inquire::Select::new(
                    "Select an option",
                    ReShadeShadersOptions::iter().collect(),
                )
                .prompt()?;

                match shader_install_option {
                    ReShadeShadersOptions::Minimal => {
                        tui::print_downloading_shaders();
                        download_minimal_reshade_shaders(data_dir).await?;
                    }
                    ReShadeShadersOptions::Select => {
                        let collections =
                            tui::prompt_select_select_shaders(SHADER_COLLECTIONS.iter().collect())?;
                        tui::print_downloading_shaders();
                        download_shader_collections(&collections, data_dir).await?;
                    }
                }

                let install_now = tui::prompt_install_shaders()?;

                if install_now {
                    if config.game_paths.is_empty() {
                        tui::print_no_game_paths();
                        return Ok(());
                    }
                    let game_paths =
                        tui::prompt_select_game_paths_shaders(config.game_paths.clone())?;
                    for game_path in &game_paths {
                        let game_path = PathBuf::from(game_path);

                        if game_path.join("gshade-shaders").exists() {
                            let replace = tui::prompt_reshade_gshade_replacement()?;
                            if replace {
                                std::fs::remove_dir_all(game_path.join("gshade-shaders"))?;
                                install_reshade_shaders(&data_dir.join("Merged"), &game_path)?;
                            } else {
                                continue;
                            }
                        } else {
                            install_reshade_shaders(&data_dir.join("Merged"), &game_path)?;
                        }
                    }
                    tui::print_shader_install_successful();
                    Ok(())
                } else {
                    tui::print_shader_download_successful();
                    Ok(())
                }
            }
            InstallOption::GShadePresets => {
                tui::print_gshade_warning();

                let open = tui::prompt_open_links()?;

                if open {
                    tui::print_gshade_file_move(data_dir);

                    let _ = open::that("https://gitlab.com/Mortalitas/GShade-C-Shaders/-/tree/main/gshade-shaders?ref_type=heads");
                    let _ = open::that("https://gitlab.com/Mortalitas/GShade-Presets/-/tree/master/FFXIV?ref_type=heads");
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

                    if game_path.join("reshade-shaders").exists() {
                        let replace = tui::prompt_gshade_reshade_replacement()?;
                        if replace {
                            std::fs::remove_dir_all(game_path.join("reshade-shaders"))?;
                            install_preset_for_game(data_dir, &game_path)?;
                        } else {
                            continue;
                        }
                    } else {
                        install_preset_for_game(data_dir, &game_path)?;
                    }
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
                uninstall(&game_path)?;

                config
                    .game_paths
                    .retain(|path| path != &game_path.to_str().unwrap().to_string());

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

async fn cli(
    subcommand: SubCommand,
    config: &mut Config,
    client: &reqwest::Client,
    data_dir: &PathBuf,
    config_path: &PathBuf,
    specific_installer: Option<String>,
) -> InquireResult<()> {
    match subcommand {
        cli::SubCommand::InstallReshade {
            vanilla,
            version,
            game,
        } => {
            download_reshade(client, data_dir, vanilla, version, &specific_installer).await?;
            if let Some(game) = game {
                let game_path = PathBuf::from(game);
                install_reshade(data_dir, &game_path, vanilla).await?;
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
            } else {
                tui::print_reshade_success_no_games(data_dir);
            }
        }
        cli::SubCommand::InstallReshadeShaders { game } => {
            tui::print_downloading_shaders();
            download_minimal_reshade_shaders(data_dir).await?;

            if let Some(game_path) = game {
                let game_path = PathBuf::from(game_path);
                install_reshade_shaders(data_dir, &game_path)?;
                tui::print_shader_install_successful();
            } else {
                tui::print_shader_download_successful();
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

            install_presets(data_dir, &presets_path, &shaders_path).await?;
            if all {
                for game_path in &config.game_paths {
                    let game_path = PathBuf::from(game_path);
                    install_preset_for_game(data_dir, &game_path)?;
                }

                tui::print_presets_success();
            } else if let Some(game) = game {
                let game_path = PathBuf::from(game);
                install_preset_for_game(data_dir, &game_path)?;

                tui::print_presets_success();
            }
        }
        cli::SubCommand::Uninstall { game } => {
            let game_path = PathBuf::from(game);
            uninstall(&game_path)?;

            config
                .game_paths
                .retain(|path| path != &game_path.to_str().unwrap().to_string());
        }
    }

    let config_str =
        toml::to_string(&config).expect("if you see this error, the toml library is broken");
    std::fs::write(config_path, config_str)?;

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
        let config = Config::default();
        let config_str =
            toml::to_string(&config).expect("if you see this error, the toml library is broken");
        std::fs::write(&config_path, config_str)?;
        config
    };
    let client = reqwest::Client::new();

    let args = cli::CliArgs::parse();
    let specific_installer = args.use_installer;

    if let Some(subcommand) = args.subcommand {
        cli(
            subcommand,
            &mut config,
            &client,
            &data_dir,
            &config_path,
            specific_installer,
        )
        .await?;
    } else {
        tui(
            &mut config,
            &client,
            &data_dir,
            &config_path,
            specific_installer,
        )
        .await?;
    }

    Ok(())
}
