# ReShader

A tool and library for installing ReShade on Linux!

**This tool is still work in progress, so expect bugs!**

## Installation

```bash
git clone https://github.com/cozygalvinism/reshader.git
cd reshader
cargo install --path .
```

## Usage

You can use ReShader in 2 ways:

1. Using the TUI (terminal user interface):

    ```bash
    reshader
    ```

2. Using the CLI (command-line interface):

    ```bash
    reshader --help
    ```

Both are completely viable options and should have similar features.

Due to how the CLI is built, you can specify the path to the presets and shaders zip file as opposed to having to put it in the data folder with a specific name. Maybe the TUI will reflect that change in the future, but for now the TUI will have this requirement.

You can provide your own path to an existing ReShade installer by passing `--use-installer <path>` to ReShader:

```bash
reshader --use-installer /home/user/ReShader-Installer.exe
```

which will skip the download of the EXE and just extract this EXE.

## Usage as library

Since ReShader is a hybrid crate, you can also write your own installer using the same functions to download and install ReShade.
This saves some time as you don't have to implement your own downloading and copying mechanisms and you can instead focus on your
user interface.

Just import `reshaderlib` into your project and use the provided functions!
