use std::path::{Path, PathBuf};

use colored::Colorize;
use inquire::{error::InquireResult, InquireError, Text};
use reshaderlib::ShaderCollection;

pub fn prompt_game_path() -> InquireResult<PathBuf> {
    let game_path = Text::new("Enter the path to your ReShade-supported game")
        .with_help_message("This is the folder containing the game executable, e.g. ~/.xlcore/ffxiv/game. Please note that ReShade might not work with unsupported games.")
        .with_default("~/.xlcore/ffxiv/game")
        .with_validator(|input: &str| {
            if input.is_empty() {
                return Ok(inquire::validator::Validation::Invalid(inquire::validator::ErrorMessage::Custom("Please enter a path!".to_string())));
            }
            if !std::path::Path::new(&input).exists() {
                return Ok(inquire::validator::Validation::Invalid(inquire::validator::ErrorMessage::Custom("The path you entered does not exist!".to_string())));
            }
            Ok(inquire::validator::Validation::Valid)
        })
        .prompt()?;
    let game_path = shellexpand::tilde(&game_path).to_string();
    Ok(std::path::Path::new(&game_path).to_path_buf())
}

pub fn prompt_open_links() -> InquireResult<bool> {
    inquire::Confirm::new("Do you want to open these download links now?")
        .with_help_message("If not, the installer will assume you have the links already present.")
        .with_default(true)
        .prompt()
}

pub fn prompt_install() -> InquireResult<bool> {
    inquire::Confirm::new("Do you want to install ReShade now?")
        .with_help_message("Answering no will return to the main menu.")
        .with_default(true)
        .prompt()
}

pub fn prompt_confirm_move() -> InquireResult<bool> {
    inquire::Confirm::new("Have you downloaded the files and put them in the correct directory?")
        .with_help_message("Answering no will cancel the installation.")
        .with_default(true)
        .prompt()
}

pub fn prompt_install_presets_for_games() -> InquireResult<bool> {
    inquire::Confirm::new("Do you want to install the preset and shaders for games now?")
        .with_help_message("This simplifies the configuration of GShade for the game.")
        .with_default(true)
        .prompt()
}

pub fn prompt_select_game_paths(paths: Vec<String>) -> InquireResult<Vec<String>> {
    inquire::MultiSelect::new(
        "Select the games you want to install the shaders and presets for",
        paths,
    )
    .prompt()
}

pub fn prompt_select_game_path_uninstall(paths: Vec<String>) -> InquireResult<PathBuf> {
    let game_path =
        inquire::Select::new("Select the game you want to uninstall ReShade from", paths)
            .prompt()?;
    let game_path = shellexpand::tilde(&game_path).to_string();
    Ok(std::path::Path::new(&game_path).to_path_buf())
}

pub fn prompt_install_shaders() -> InquireResult<bool> {
    inquire::Confirm::new("Do you want to install the shaders now?")
        .with_help_message("Answering no will return to the main menu.")
        .with_default(true)
        .prompt()
}

pub fn prompt_select_game_paths_shaders(paths: Vec<String>) -> InquireResult<Vec<String>> {
    inquire::MultiSelect::new(
        "Select the games you want to install the shaders for",
        paths,
    )
    .prompt()
}

pub fn prompt_select_select_shaders(
    collections: Vec<&ShaderCollection>,
) -> InquireResult<Vec<&ShaderCollection>> {
    inquire::MultiSelect::new("Select the shaders you want to install", collections).prompt()
}

pub fn prompt_gshade_reshade_replacement() -> InquireResult<bool> {
    inquire::Confirm::new("GShade and ReShade shaders are not compatible with each other. Do you want to replace ReShade with GShade?")
        .with_help_message("Answering no will continue to the next selected game.")
        .with_default(true)
        .prompt()
}

pub fn prompt_reshade_gshade_replacement() -> InquireResult<bool> {
    inquire::Confirm::new("GShade and ReShade shaders are not compatible with each other. Do you want to replace GShade with ReShade?")
        .with_help_message("Answering no will continue to the next selected game.")
        .with_default(true)
        .prompt()
}

pub fn print_reshade_success() {
    println!();
    println!("{}", "ReShade installed successfully! Please restart your game to enable it. Note that this installation did not install any presets or shaders!".bright_green());
    println!();
}

pub fn print_gshade_warning() {
    println!();
    println!("{}", "As it is not allowed to redistribute or automatically download presets and shaders, you will have to download them manually.".cyan());
    println!("{}", "However, ReShader can open your browser and take you to the correct links for you to download these files yourself.".cyan());
    println!("{}", "A fair warning though: GPosers might take down the download links at any time, so you might have to find the files yourself.".yellow());
    println!();
}

pub fn print_gshade_file_move(directory: &Path) {
    println!();
    println!(
        "{} {} {}",
        "After you have downloaded the files, please put them in the".cyan(),
        directory.to_str().unwrap().white().bold(),
        "directory, named \"shaders.zip\" and \"presets.zip\".".cyan()
    );
    println!();
}

pub fn print_gshade_hint() {
    println!();
    println!(
        "{}",
        "If your browser does not open, please open the following links manually:".cyan()
    );
    println!(
        "{}\n{}",
        "https://gitlab.com/Mortalitas/GShade-C-Shaders/-/tree/main/gshade-shaders?ref_type=heads"
            .white()
            .bold(), 
        "https://gitlab.com/Mortalitas/GShade-Presets/-/tree/master/FFXIV?ref_type=heads"
            .white()
            .bold(), 
    );
    println!();
}

pub fn print_presets_success_no_games(directory: &Path) {
    println!();
    println!(
        "{} {} {}",
        "Installation complete! GShade's presets and shaders are located at".bright_green(),
        directory.to_str().unwrap().white().bold(),
        ".".bright_green()
    );
    println!("{}", "In order to install them for your game, you will need to configure your ReShade to include the shaders folder as the effects and texture path (it contains 2 subdirectories called Shaders and ComputeShaders, which will need to be added to the effects search path and one subdirectory called Textures, which will need to be added to the texture search path).".bright_green());
    println!();
}

pub fn print_reshade_success_no_games(directory: &Path) {
    println!();
    println!(
        "{} {} {}",
        "Installation complete! ReShade is located at".bright_green(),
        directory.to_str().unwrap().white().bold(),
        ".".bright_green()
    );
    println!("{}", "In order to install it for your game, you will need to re-run this installer and provide a game path.".bright_green());
    println!();
}

pub fn print_presets_success() {
    println!();
    println!("{}", "Installation complete!".bright_green());
    println!();
}

pub fn print_no_game_paths() {
    println!();
    println!(
        "{}",
        "No game paths with an installed ReShade or GShade found.".bright_red()
    );
    println!();
}

pub fn print_no_home_dir() {
    println!();
    println!("{}", "Could not find your home directory. Please make sure you have a home directory and try again.".bright_red());
    println!();
}

pub fn print_config_deserialization_error() {
    println!();
    println!(
        "{}",
        "Could not deserialize the configuration file. Please make sure it is valid and try again."
            .bright_red()
    );
    println!();
}

pub fn print_error(error: InquireError) {
    println!();
    println!("{}", format!("An error occurred: {error}").bright_red());
    println!();
}

pub fn print_downloading_shaders() {
    println!();
    println!("{}", "Downloading shaders...".cyan());
    println!();
}

pub fn print_shader_install_successful() {
    println!();
    println!(
        "{}",
        "Successfully installed ReShade shaders!".bright_green()
    );
    println!();
}

pub fn print_shader_download_successful() {
    println!();
    println!("{}", "Successfully downloaded ReShade shaders! To install them, run this option again and select a game!".bright_green());
    println!();
}
