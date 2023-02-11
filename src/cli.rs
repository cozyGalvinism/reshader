#[derive(Debug, clap::Parser)]
#[command(author, version, about)]
pub struct CliArgs {
    /// What ReShader should do
    #[clap(subcommand)]
    pub subcommand: Option<SubCommand>,

    /// Use a specific ReShade installer at this path
    #[arg(short, long)]
    pub use_installer: Option<String>,
}

#[derive(Debug, clap::Subcommand)]
pub enum SubCommand {
    /// Install ReShade for a game
    ///
    /// If use_installer is specified, the arguments for version and vanilla are ignored
    InstallReshade {
        /// Install a version of ReShade that has no support for addons
        #[arg(long)]
        vanilla: bool,
        /// Install a specific version of ReShade (default: latest)
        #[arg(short, long)]
        version: Option<String>,
        /// Install the ReShade library for this game. If this isn't set, the installer will only download ReShade.
        #[arg(short, long)]
        game: Option<String>,
    },
    /// Install GShade presets and shaders for a game. If no game is specified and all is not set, the presets and shaders will only be downloaded.
    InstallPresets {
        /// Install the presets and shaders for all games
        #[arg(short, long)]
        all: bool,
        /// Install the presets and shaders for a specific game (if all is specified, this argument is ignored)
        #[arg(short, long)]
        game: Option<String>,
        /// Location of the GShade presets zip file
        #[arg(short, long, required = true)]
        presets: String,
        /// Location of the GShade shaders zip file
        #[arg(short, long, required = true)]
        shaders: String,
    },
    /// Uninstall ReShade or GShade from a game
    Uninstall {
        /// Uninstall from this game
        #[arg(short, long)]
        game: String,
    },
}
