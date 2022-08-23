# Veilid

## Introduction

## Obtaining the source code

```shell
git clone --recurse-submodules git@gitlab.hackers.town:veilid/veilid.git
```

## Dependencies

### GNU/Linux

Development of Veilid on GNU/Linux requires a Debian variant such as Debian
itself, Ubuntu or Mint. Pull requests to support other distributions would be
welcome!

Running the setup script requires:
* Android SDK and NDK
* Rust

You may decide to use Android Studio [here](https://developer.android.com/studio) 
to maintain your Android dependencies. If so, use the dependency manager 
within your IDE. If you plan on using Flutter for Veilid development, the Android Studio
method is highly recommended as you may run into path problems with the 'flutter' 
command line without it. If you do so, you may skip to 
[Run Veilid setup script](#Run Veilid setup script).

* build-tools;30.0.3
* ndk;22.0.7026061
* cmake;3.22.1

#### Setup Dependencies using the CLI

Otherwise, you may choose to use Android `sdkmanager`. Follow the installation
instructions for `sdkmanager`
[here](https://developer.android.com/studio/command-line/sdkmanager), then use
the command line to install the requisite package versions:

```shell
sdkmanager --install "build-tools;30.0.3"
sdkmanager --install "ndk;22.0.7026061"
sdkmanager --install "cmake;3.22.1"
```

Export environment variables and add the Android SDK platform-tools directory to
your path.

```shell
cat << EOF >> ~/.profile 
export ANDROID_SDK_ROOT=<path to sdk>
export ANDROID_NDK_HOME=$ANDROID_SDK_ROOT/ndk/22.0.7026061
export PATH=\$PATH:$ANDROID_SDK_ROOT/platform-tools
EOF
```

#### Run Veilid setup script

Now you may run the Linux setup script to check your development environment and
pull the remaining Rust dependencies:

```shell
./setup_linux.sh
```

#### Run the veilid-flutter setup script (optional)

If you are developing Flutter applications or the flutter-veilid portion, you should
install Android Studio, and run the flutter setup script:

```shell
cd veilid-flutter
./setup_flutter.sh
```


### macOS

Development of Veilid on MacOS is possible on both Intel and ARM hardware.

Development requires:
* Android Studio 
* Xcode, preferably latest version
* Homebrew [here](https://brew.sh)
* Android SDK and NDK
* Rust

You will need to use Android Studio [here](https://developer.android.com/studio) 
to maintain your Android dependencies. Use the SDK Manager in the IDE to install the following packages (use package details view to select version):
* Android SDK Build Tools (30.0.3)
* NDK (Side-by-side) (22.0.7026061)
* Cmake (3.22.1)
* Android SDK Command Line Tools (latest) (7.0/latest)

#### Setup command line environment

Export environment variables and add the Android SDK platform-tools directory to
your path.

```shell
cat << EOF >> ~/.zshenv
export ANDROID_SDK_ROOT=$HOME/Library/Android/sdk
export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/22.0.7026061
export PATH=\$PATH:$HOME/Library/Android/sdk/platform-tools
EOF
```

#### Run Veilid setup script

Now you may run the MacOS setup script to check your development environment and
pull the remaining Rust dependencies:

```shell
./setup_macos.sh
```

#### Run the veilid-flutter setup script (optional)

If you are developing Flutter applications or the flutter-veilid portion, you should
install Android Studio, and run the flutter setup script:

```shell
cd veilid-flutter
./setup_flutter.sh
```

### Windows

**TODO**

## Veilid Server

In order to run the `veilid-server` locally:

```shell
cd ./veilid-server
cargo run
```

In order to see what options are available:

```shell
cargo run -- --help
```

## Veilid CLI

In order to connect to your local `veilid-server`:

```shell
cd ./veilid-cli
cargo run
```

Similar to `veilid-server`, you may see CLI options by typing:

```shell
cargo run -- --help
```
