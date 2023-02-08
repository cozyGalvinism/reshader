use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use colored::Colorize;
use config::Config;
use dircpy::{copy_dir, CopyBuilder};
use inquire::{error::InquireResult, Confirm, Select, Text, MultiSelect};
use strum::{EnumIter, IntoEnumIterator};

mod config;

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

async fn download_file(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get(url).await?.bytes().await?;
    let mut out = tokio::fs::File::create(path).await?;
    let mut reader = tokio::io::BufReader::new(resp.as_ref());
    tokio::io::copy(&mut reader, &mut out).await?;
    Ok(())
}

async fn get_latest_reshade_version(vanilla: bool) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let tags = client
        .get("https://api.github.com/repos/crosire/reshade/tags")
        .header(
            reqwest::header::USER_AGENT,
            format!("reshader/{}", APP_VERSION),
        )
        .send()
        .await?
        .json::<Vec<serde_json::Value>>()
        .await?;
    let mut tags = tags
        .iter()
        .map(|tag| tag["name"].as_str().unwrap().trim_start_matches('v'))
        .collect::<Vec<_>>();
    tags.sort_by(|a, b| {
        let a = semver::Version::parse(a).unwrap();
        let b = semver::Version::parse(b).unwrap();
        a.cmp(&b)
    });
    let latest = tags.last().unwrap().trim_start_matches('v');
    if vanilla {
        Ok(format!(
            "http://static.reshade.me/downloads/ReShade_Setup_{}.exe",
            latest
        ))
    } else {
        Ok(format!(
            "http://static.reshade.me/downloads/ReShade_Setup_{}_Addon.exe",
            latest
        ))
    }
}

async fn install_reshade(config: &mut Config, vanilla: bool) -> InquireResult<()> {
    let game_path = Text::new("Enter the path to your ReShade-supported game")
        .with_help_message("This is the folder containing the game executable, e.g. ~/.xlcore/ffxiv/game. Please note that ReShade might not work with unsupported games.")
        .with_default("~/.xlcore/ffxiv/game")
        .prompt()?;
    let game_path = shellexpand::tilde(&game_path).to_string();
    let game_path = std::path::Path::new(&game_path);

    let tmp =
        tempdir::TempDir::new("reshader_downloads").expect("Could not create temporary directory");
    let reshade_url = get_latest_reshade_version(vanilla)
        .await
        .expect("Could not get latest ReShade version");
    let reshade_path = tmp.path().join("reshade.exe");
    let d3dcompiler_path = tmp.path().join("d3dcompiler_47.dll");
    download_file(&reshade_url, reshade_path.to_str().unwrap())
        .await
        .expect("Could not download ReShade installer");
    download_file(
        "https://lutris.net/files/tools/dll/d3dcompiler_47.dll",
        d3dcompiler_path.to_str().unwrap(),
    )
    .await
    .expect("Could not download d3dcompiler_47.dll");

    let mut cmd = std::process::Command::new("7z");
    let cmd = cmd
        .arg("x")
        .arg(format!("-o{}", tmp.path().to_str().unwrap()))
        .arg(reshade_path.to_str().unwrap())
        .arg("ReShade64.dll");
    let output = cmd.output().expect("Could not extract ReShade installer");
    if !output.status.success() {
        println!(
            "Could not extract ReShade installer: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    // Move ReShade64.dll to game directory
    let reshade_dll = tmp.path().join("ReShade64.dll");
    std::fs::copy(reshade_dll, game_path.join("dxgi.dll"))
        .expect("Could not copy ReShade64.dll to game directory");
    std::fs::copy(d3dcompiler_path, game_path.join("d3dcompiler_47.dll"))
        .expect("Could not copy d3dcompiler_47.dll to game directory");

    println!();
    println!("{}", "ReShade installed successfully! Please restart your game to enable it. Note that this installation did not install any presets or shaders!".bright_green());
    println!();

    if config.game_paths.contains(&game_path.to_str().unwrap().to_string()) {
        return Ok(());
    }

    config
        .game_paths
        .push(game_path.to_str().unwrap().to_string());

    Ok(())
}

async fn install_presets(config: &mut Config, directory: PathBuf) -> InquireResult<()> {
    println!();
    println!("{}", "As it is not allowed to redistribute or automatically download presets and shaders, you will have to download them manually.".cyan());
    println!("{}", "However, ReShader can open your browser and take you to the correct links for you to download these files yourself.".cyan());
    println!("{}", "A fair warning though: GPosers might take down the download links at any time, so you might have to find the files yourself.".yellow());
    let open = inquire::Confirm::new("Do you want to open these download links now?")
        .with_help_message("If not, the installer will assume you have the links already present.")
        .with_default(true)
        .prompt()?;

    if open {
        println!(
            "{} {} {}",
            "After you have downloaded the files, please put them in the".cyan(),
            directory.to_str().unwrap().white().bold(),
            "directory, named \"shaders.zip\" and \"presets.zip\".".cyan()
        );

        let mut cmd = std::process::Command::new("xdg-open");
        let cmd = cmd
            // kind of ironic :)
            .arg("https://github.com/HereInPlainSight/gshade_installer/blob/master/gshade_installer.sh#L352");
        let output = cmd.output().expect("Could not open browser");
        if !output.status.success() {
            println!(
                "Could not open browser: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(());
        }
    }

    let done = Confirm::new("Have you downloaded the files and put them in the correct directory?")
        .with_help_message("Answering no will cancel the installation.")
        .with_default(true)
        .prompt()?;

    if !done {
        return Ok(());
    }

    let presets_path = directory.join("presets.zip");
    let file = std::fs::File::open(presets_path).expect("unable to open presets file");
    let mut presets_zip =
        zip::read::ZipArchive::new(file).expect("unable to read presets zip file");
    presets_zip
        .extract(&directory)
        .expect("unable to extract presets zip file");

    CopyBuilder::new(
        directory.join("GShade-Presets-master"),
        directory.join("reshade-presets"),
    )
    .overwrite(true)
    .run()
    .expect("unable to copy presets");
    std::fs::remove_dir_all(directory.join("GShade-Presets-master"))
        .expect("unable to remove presets directory");

    let shaders_path = directory.join("shaders.zip");
    let file = std::fs::File::open(shaders_path).expect("unable to open shaders file");
    let mut shaders_zip =
        zip::read::ZipArchive::new(file).expect("unable to read shaders zip file");
    shaders_zip
        .extract(&directory)
        .expect("unable to extract shaders zip file");

    CopyBuilder::new(
        directory.join("gshade-shaders"),
        directory.join("reshade-shaders"),
    )
    .overwrite(true)
    .run()
    .expect("unable to copy presets");
    std::fs::remove_dir_all(directory.join("gshade-shaders"))
        .expect("unable to remove presets directory");

    if config.game_paths.is_empty() {
        println!();
        println!(
            "{} {} {}",
            "Installation complete! GShade's presets and shaders are located at"
                .bright_green(),
            directory.to_str().unwrap().white().bold(),
            ".".bright_green()
        );
        println!("{}", "In order to install them for your game, you will need to configure your ReShade to include the shaders folder as the effects and texture path (it contains 2 subdirectories called Shaders and ComputeShaders, which will need to be added to the effects search path and one subdirectory called Textures, which will need to be added to the texture search path).".bright_green());
        println!();
        return Ok(());
    }

    let install_for_games =
        Confirm::new("Do you want to install the preset and shaders for games now?")
            .with_help_message("This simplifies the configuration of GShade for the game.")
            .with_default(true)
            .prompt()?;

    if !install_for_games {
        println!();
        println!(
            "{} {} {}",
            "Installation complete! GShade's presets and shaders are located at"
                .bright_green(),
            directory.to_str().unwrap().white().bold(),
            ".".bright_green()
        );
        println!("{}", "In order to install them for your game, you will need to configure your ReShade to include the shaders folder as the effects and texture path (it contains 2 subdirectories called Shaders and ComputeShaders, which will need to be added to the effects search path and one subdirectory called Textures, which will need to be added to the texture search path).".bright_green());
        println!();
        return Ok(());
    }

    let game_paths = MultiSelect::new("Select the games you want to install the shaders and presets for", config.game_paths.clone())
            .prompt()?;
    for game_path in &game_paths {
        let target_preset_path = PathBuf::from(game_path).join("reshade-presets");
        let target_shaders_path = PathBuf::from(game_path).join("reshade-shaders");

        if std::fs::read_link(target_preset_path).is_ok()
            || std::fs::read_link(target_shaders_path).is_ok()
        {
            continue;
        }

        std::os::unix::fs::symlink(
            directory.join("reshade-presets"),
            PathBuf::from(game_path).join("reshade-presets"),
        )
        .expect("unable to create symlink");
        std::os::unix::fs::symlink(
            directory.join("reshade-shaders"),
            PathBuf::from(game_path).join("reshade-shaders"),
        )
        .expect("unable to create symlink");
    }

    println!();
    println!(
        "{}",
        "Installation complete! You now need to configure your ReShade as follows:".bright_green()
    );
    println!();
    println!("\t{}", "Set your \"effect search path\" to \"./reshade-shaders/Shaders\" and \"./reshade-shaders/ComputeShaders\"".bright_green());
    println!(
        "\t{}",
        "Set your \"textures search path\" to \"./reshade-shaders/Textures\"".bright_green()
    );
    println!("\t{}", "In order to use GShade presets, you might need to browse to them in your ReShade UI. You can find them in your game's directory!".bright_green());
    println!();

    Ok(())
}

fn uninstall(config: &mut Config) -> InquireResult<()> {
    if config.game_paths.is_empty() {
        println!("{}", "No games with ReShade installed found.".bright_red());
        return Ok(());
    }

    let game_path = Select::new(
        "Select the game you want to uninstall ReShade from",
        config.game_paths.clone(),
    )
    .prompt()?;

    let dxgi_path = PathBuf::from(&game_path).join("dxgi.dll");
    let d3dcompiler_path = PathBuf::from(&game_path).join("d3dcompiler_47.dll");
    let presets_path = PathBuf::from(&game_path).join("reshade-presets");
    let shaders_path = PathBuf::from(&game_path).join("reshade-shaders");

    if dxgi_path.exists() {
        std::fs::remove_file(dxgi_path).expect("Failed to remove dxgi.dll");
    }
    if d3dcompiler_path.exists() {
        std::fs::remove_file(d3dcompiler_path).expect("Failed to remove d3dcompiler.dll");
    }
    if presets_path.exists() {
        std::fs::remove_dir_all(presets_path).expect("Failed to remove presets.zip");
    }
    if shaders_path.exists() {
        std::fs::remove_dir_all(shaders_path).expect("Failed to remove shaders.zip");
    }

    config.game_paths.retain(|path| path != &game_path);

    Ok(())
}

#[tokio::main]
async fn main() -> InquireResult<()> {
    if !cfg!(target_os = "linux") {
        println!("This installer is only supported on Linux");
        return Ok(());
    }

    let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .expect("Could not find home directory");
    std::fs::create_dir_all(dirs.config_dir()).expect("Could not create config directory");
    std::fs::create_dir_all(dirs.data_dir()).expect("Could not create data directory");

    let config_path = dirs.config_dir().join("config.toml");
    let mut config = if config_path.exists() {
        let config_str = std::fs::read_to_string(&config_path).expect("Could not read config file");
        toml::from_str(&config_str).expect("Could not parse config file")
    } else {
        let config = config::Config::default();
        let config_str = toml::to_string(&config).expect("Could not serialize config");
        std::fs::write(&config_path, config_str).expect("Could not write config file");
        config
    };

    let mut cmd = std::process::Command::new("7z");
    cmd.output().expect("Please make sure 7z is installed");

    loop {
        let install_option =
            Select::new("Select an option", InstallOption::iter().collect()).prompt()?;

        match install_option {
            InstallOption::ReShade => install_reshade(&mut config, false).await?,
            InstallOption::ReShadeVanilla => install_reshade(&mut config, true).await?,
            InstallOption::Presets => {
                install_presets(&mut config, dirs.data_dir().to_path_buf()).await?
            }
            InstallOption::Uninstall => uninstall(&mut config)?,
            InstallOption::Quit => break,
        }

        let config_str = toml::to_string(&config).expect("Could not serialize config");
        std::fs::write(&config_path, config_str).expect("Could not write config file");
    }

    Ok(())
}
