#![deny(missing_docs)]

//! # reshaderlib
//!
//! This library contains the common code for the ReShader installer.
//!
//! You can use this crate as a base to create your own ReShade installer.
//!
//! ## Examples
//!
//! For examples, please look at the [ReShader installer](https://github.com/cozyGalvinism/reshader).

use dircpy::CopyBuilder;
use lazy_static::lazy_static;
use std::{
    fmt::{Display, Formatter},
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use crate::prelude::*;

/// Common ReShader types and functions
pub mod prelude;

static LIB_VERSION: &str = env!("CARGO_PKG_VERSION");
static DEFAULT_INI: &str = include_str!("../../reshade.example.ini");
static PACKAGES_INI: &str = include_str!("../../shader-list/EffectPackages.ini");

/// Contains shader collections parsed from the EffectPackages.ini file
pub mod ini_packages {}

lazy_static! {
    /// The default ReShade shader list
    pub static ref SHADER_COLLECTIONS: Vec<ShaderCollection> = {
        ini::Ini::load_from_str_noescape(PACKAGES_INI)
            .unwrap()
            .iter()
            .map(|(_, section)| {
                let enabled = section.get("Enabled").unwrap_or("0").parse::<u8>().unwrap_or(0) == 1;
                let required = section.get("Required").unwrap_or("0").parse::<u8>().unwrap_or(0) == 1;
                let name = section.get("PackageName").unwrap().to_string();
                let description = section.get("PackageDescription").unwrap().to_string();
                let install_path = section.get("InstallPath").unwrap()
                    [2..]
                    .replace('\\', "/")
                    .replace("reshade-shaders", "Merged");
                let texture_install_path = section.get("TextureInstallPath").unwrap()
                    [2..]
                    .replace('\\', "/")
                    .replace("reshade-shaders", "Merged");
                let download_url = section.get("DownloadUrl").unwrap().to_string();

                ShaderCollection::new(
                    enabled,
                    required,
                    &name,
                    &description,
                    &install_path,
                    &texture_install_path,
                    &download_url,
                )
            })
            .collect()
    };
}

/// A shader collection
#[derive(Debug)]
pub struct ShaderCollection {
    /// Whether the shader collection is enabled
    pub enabled: bool,
    /// Whether the shader collection is required
    pub required: bool,
    /// The name of the shader collection
    pub name: String,
    /// The description of the shader collection
    pub description: String,
    /// The path to install the shaders to
    pub install_path: String,
    /// The path to install the textures to
    pub texture_install_path: String,
    /// The URL to download the shader collection from
    pub download_url: String,
}

impl ShaderCollection {
    /// Creates a new shader collection
    pub fn new(
        enabled: bool,
        required: bool,
        name: &str,
        description: &str,
        install_path: &str,
        texture_install_path: &str,
        download_url: &str,
    ) -> Self {
        Self {
            enabled,
            required,
            name: name.to_string(),
            description: description.to_string(),
            install_path: install_path.to_string(),
            texture_install_path: texture_install_path.to_string(),
            download_url: download_url.to_string(),
        }
    }

    /// Downloads the shader collection to the given directory
    pub async fn download(&self, target_directory: &Path) -> ReShaderResult<()> {
        if !target_directory.exists() {
            std::fs::create_dir(target_directory)?;
        }
        let target_path = target_directory.join(format!("{}.zip", &self.name));
        let client = reqwest::Client::new();
        download_file(&client, &self.download_url, &target_path).await?;

        Ok(())
    }

    /// Unpacks the shader collection to the given directory and return the name of the root directory of the zip file
    ///
    /// If there is no root directory in the zip file, one will be created.
    pub fn unpack(&self, target_directory: &Path) -> ReShaderResult<String> {
        let zip_path = target_directory.join(format!("{}.zip", &self.name));
        let mut zip_file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(&mut zip_file)?;
        let mut root_dir = None;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = target_directory.join(file.enclosed_name().unwrap());
            if (*file.name()).ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
            if root_dir.is_none() {
                root_dir = Some(file.name().to_string());
            }
        }
        if let Some(root_dir) = root_dir {
            Ok(root_dir)
        } else {
            let root_dir = format!("{}.zip", &self.name);
            std::fs::create_dir(target_directory.join(&root_dir))?;
            Ok(root_dir)
        }
    }
}

impl Display for ShaderCollection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name,)
    }
}

/// Downloads a file from the given URL to the given path
pub async fn download_file(client: &reqwest::Client, url: &str, path: &Path) -> ReShaderResult<()> {
    let resp = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("reshader/{LIB_VERSION}"),
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

/// Downloads the specified shader collections to the given directory
pub async fn download_shader_collections(
    collections: &[&ShaderCollection],
    directory: &Path,
) -> ReShaderResult<()> {
    let zip_directory = directory.join("zips");

    if zip_directory.exists() {
        std::fs::remove_dir_all(&zip_directory)?;
    }

    if !zip_directory.exists() {
        std::fs::create_dir(&zip_directory)?;
    }

    for collection in collections {
        collection.download(&zip_directory).await?;
        let root_dir = collection.unpack(&zip_directory)?;

        let repo_directory = zip_directory.join(root_dir);
        let repo_shader_directory = repo_directory.join("Shaders");
        let repo_texture_directory = repo_directory.join("Textures");
        let target_shader_directory = directory.join(&collection.install_path);
        let target_texture_directory = directory.join(&collection.texture_install_path);
        if !target_shader_directory.exists() {
            std::fs::create_dir_all(&target_shader_directory)?;
        }
        if !target_texture_directory.exists() {
            std::fs::create_dir_all(&target_texture_directory)?;
        }

        if repo_shader_directory.exists() {
            let builder = CopyBuilder::new(&repo_shader_directory, &target_shader_directory);
            builder.overwrite(true).run()?;
        }

        if repo_texture_directory.exists() {
            let builder = CopyBuilder::new(&repo_texture_directory, &target_texture_directory);
            builder.overwrite(true).run()?;
        }
    }

    Ok(())
}

/// Downloads the minimal ReShade shaders and textures to a directory
pub async fn download_minimal_reshade_shaders(directory: &Path) -> ReShaderResult<()> {
    let minimal_shaders = SHADER_COLLECTIONS
        .iter()
        .filter(|c| c.enabled)
        .collect::<Vec<_>>();
    download_shader_collections(&minimal_shaders, directory).await?;

    Ok(())
}

/// Installs ReShade shaders and textures to a game directory by symlinking them
///
/// This function will create a symlink called `reshade-shaders` in the game directory
pub fn install_reshade_shaders(directory: &Path, game_path: &Path) -> ReShaderResult<()> {
    let target_path = game_path.join("reshade-shaders");
    // if target_path exists and is not a symlink, return an error
    if target_path.exists() && std::fs::read_link(&target_path).is_err() {
        return Err(ReShaderError::Symlink(
            directory.to_str().unwrap().to_string(),
            target_path.to_str().unwrap().to_string(),
            "Directory already exists".to_string(),
        ));
    } else if target_path.exists() && std::fs::read_link(&target_path).is_ok() {
        return Ok(());
    }

    std::os::unix::fs::symlink(directory, &target_path)?;

    Ok(())
}

/// Fetches the latest ReShade version from GitHub.
///
/// Alternatively, if `version` is provided, it will return that version.
/// Please note that there is no check to see if the version is valid or not.
pub async fn get_latest_reshade_version(
    client: &reqwest::Client,
    version: Option<String>,
    vanilla: bool,
) -> ReShaderResult<String> {
    let version = if let Some(version) = version {
        version
    } else {
        let tags = client
            .get("https://api.github.com/repos/crosire/reshade/tags")
            .header(
                reqwest::header::USER_AGENT,
                format!("reshader/{LIB_VERSION}"),
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
            "https://reshade.me/downloads/ReShade_Setup_{version}.exe"
        ))
    } else {
        Ok(format!(
            "https://reshade.me/downloads/ReShade_Setup_{version}_Addon.exe"
        ))
    }
}

/// Downloads ReShade and d3dcopmiler_47.dll to the given directory.
///
/// If `specific_installer` is provided, it will use that installer instead of downloading the latest version.
///
/// If `version` is provided, it will use that version instead of the latest version.
///
/// If `vanilla` is true, it will download the vanilla version of ReShade instead of the addon version.
pub async fn download_reshade(
    client: &reqwest::Client,
    target_directory: &Path,
    vanilla: bool,
    version: Option<String>,
    specific_installer: &Option<String>,
) -> ReShaderResult<()> {
    let tmp = tempdir::TempDir::new("reshader_downloads")?;

    let reshade_path = if let Some(specific_installer) = specific_installer {
        PathBuf::from(specific_installer)
    } else {
        let reshade_url = get_latest_reshade_version(client, version, vanilla)
            .await
            .expect("Could not get latest ReShade version");
        let reshade_path = tmp.path().join("reshade.exe");

        download_file(client, &reshade_url, &reshade_path).await?;
        reshade_path
    };

    let d3dcompiler_path = tmp.path().join("d3dcompiler_47.dll");
    download_file(
        client,
        "https://lutris.net/files/tools/dll/d3dcompiler_47.dll",
        &d3dcompiler_path,
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
        target_directory.join("ReShade64.Vanilla.dll")
    } else {
        target_directory.join("ReShade64.Addon.dll")
    };
    std::fs::write(reshade_dll, buf)?;

    std::fs::copy(
        d3dcompiler_path,
        target_directory.join("d3dcompiler_47.dll"),
    )?;

    Ok(())
}

/// Installs ReShade to the given game directory by symlinking the ReShade dll
/// and d3dcompiler_47.dll to the game directory.
///
/// Depending on the `vanilla` parameter, it will symlink the vanilla or addon version of ReShade.
pub async fn install_reshade(
    data_dir: &Path,
    game_path: &Path,
    vanilla: bool,
) -> ReShaderResult<()> {
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

    let ini_path = game_path.join("ReShade.ini");
    if !ini_path.exists() {
        std::fs::write(ini_path, DEFAULT_INI)?;
    }

    Ok(())
}

/// Installs GShade presets and shaders to the given directory.
///
/// This does **not** download the presets and shaders, it just extracts them
/// from the given zip files.
pub async fn install_presets(
    directory: &PathBuf,
    presets_path: &PathBuf,
    shaders_path: &PathBuf,
) -> ReShaderResult<()> {
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

    let intermediate_path = directory.join("reshade-shaders").join("Intermediate");
    if !intermediate_path.exists() {
        std::fs::create_dir(intermediate_path)?;
    }

    Ok(())
}

/// Uninstalls ReShade from the given game directory by removing the ReShade dll
/// (dxgi.dll) and d3dcompiler_47.dll.
///
/// INI files are not removed.
pub fn uninstall(game_path: &Path) -> ReShaderResult<()> {
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

    Ok(())
}

/// Installs the GShade presets and shaders to the given game directory by symlinking
pub fn install_preset_for_game(data_dir: &Path, game_path: &Path) -> ReShaderResult<()> {
    let target_preset_path = PathBuf::from(game_path).join("gshade-presets");
    let target_shaders_path = PathBuf::from(game_path).join("gshade-shaders");

    if std::fs::read_link(&target_preset_path).is_ok()
        || std::fs::read_link(&target_shaders_path).is_ok()
    {
        return Ok(());
    }

    std::os::unix::fs::symlink(data_dir.join("reshade-presets"), target_preset_path)?;
    std::os::unix::fs::symlink(data_dir.join("reshade-shaders"), target_shaders_path)?;
    Ok(())
}
