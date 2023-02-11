use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use dircpy::CopyBuilder;

use crate::prelude::*;

pub mod prelude;

static APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn download_file(client: &reqwest::Client, url: &str, path: &str) -> ReShaderResult<()> {
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

    Ok(())
}

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

    Ok(())
}

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

pub fn install_preset_for_game(data_dir: &Path, game_path: &Path) -> ReShaderResult<()> {
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
