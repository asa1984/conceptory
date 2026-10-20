//! CLI for the norms that conceptory distributes.

use std::{
    fmt, fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use conceptory_okf::{Diagnostic, Schemas};

#[derive(Parser)]
#[command(version, about = "conceptory が配布する規約を扱う")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "リポジトリのファイル名の規約を検査する")]
    Files(FilesCommand),
    #[command(
        subcommand,
        about = "OKF bundle を検査し、閲覧し、文書の frontmatter から textlint の辞書を生成する"
    )]
    Okf(OkfCommand),
}

#[derive(Subcommand)]
enum FilesCommand {
    #[command(about = "YAML ファイルの拡張子が `.yaml` であることを検査する")]
    Check {
        #[arg(
            default_value = ".",
            help = "検査するディレクトリ。`.gitignore` で除外されたファイルは検査しない"
        )]
        root: PathBuf,
        #[arg(
            long = "exclude",
            value_name = "GLOB",
            help = "検査から外すファイルの glob パターン。ルートからの相対パスと照合する"
        )]
        excludes: Vec<String>,
    },
}

#[derive(Subcommand)]
enum OkfCommand {
    #[command(about = "OKF bundle を検査する")]
    Check {
        #[arg(
            required = true,
            help = "OKF bundle のルート。複数を渡すと、用語と避ける語の重複を OKF bundle \
                    をまたいで検査する"
        )]
        roots: Vec<PathBuf>,
        #[arg(
            long = "schemas",
            value_name = "DIR",
            help = "組み込みのスキーマに重ねて読み込む `*.schema.json` のディレクトリ"
        )]
        schemas: Vec<PathBuf>,
    },
    #[command(about = "各ディレクトリの index.md を生成して書き込む")]
    Index {
        #[arg(required = true, help = "OKF bundle のルート")]
        roots: Vec<PathBuf>,
    },
    #[command(about = "frontmatter の terms を一覧にする")]
    Terms {
        #[arg(required = true, help = "OKF bundle のルート")]
        roots: Vec<PathBuf>,
        #[arg(
            long,
            value_name = "FILE",
            help = "一覧の代わりに prh の辞書をこのパスに書き出す"
        )]
        prh: Option<PathBuf>,
    },
    #[command(
        about = "frontmatter の vocabulary から textlint-rule-preset-ai-words-ja の辞書を書き出す"
    )]
    Vocabulary {
        #[arg(required = true, help = "OKF bundle のルート")]
        roots: Vec<PathBuf>,
        #[arg(long, value_name = "FILE", help = "辞書の書き出し先")]
        out: PathBuf,
    },
    #[command(about = "OKF bundle を閲覧する viewer \
                       をローカルで配信する。ファイルを編集すると表示に反映される")]
    View {
        #[arg(required = true, help = "OKF bundle のルート")]
        roots: Vec<PathBuf>,
        #[arg(
            long = "schemas",
            value_name = "DIR",
            help = "組み込みのスキーマに重ねて読み込む `*.schema.json` のディレクトリ"
        )]
        schemas: Vec<PathBuf>,
        #[arg(
            long,
            default_value_t = 7878,
            help = "待ち受けるポート。0 を渡すと空いているポートを使う"
        )]
        port: u16,
        #[arg(long, help = "起動後にブラウザを開かない")]
        no_open: bool,
        #[arg(
            long = "allow-origin",
            value_name = "ORIGIN",
            help = "自身のほかに API への接続を許可する Origin。viewer の開発用の dev server \
                    を渡す"
        )]
        allowed_origins: Vec<String>,
    },
}

/// A failure while running a command.
#[derive(Debug)]
enum Error {
    Files(conceptory_files::Error),
    Okf(conceptory_okf::Error),
    Server(conceptory_server::Error),
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Files(error) => error.fmt(f),
            Self::Okf(error) => error.fmt(f),
            Self::Server(error) => error.fmt(f),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl From<conceptory_files::Error> for Error {
    fn from(error: conceptory_files::Error) -> Self {
        Self::Files(error)
    }
}

impl From<conceptory_okf::Error> for Error {
    fn from(error: conceptory_okf::Error) -> Self {
        Self::Okf(error)
    }
}

impl From<conceptory_server::Error> for Error {
    fn from(error: conceptory_server::Error) -> Self {
        Self::Server(error)
    }
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Files(command) => run_files(command),
        Command::Okf(command) => run_okf(command),
    };
    result.unwrap_or_else(|error| {
        // If stderr is unwritable there is no other way to report the error
        let _ = writeln!(io::stderr(), "conceptory: {error}");
        ExitCode::from(2)
    })
}

/// Runs a `conceptory files` subcommand.
///
/// # Errors
///
/// Returns an error if the files cannot be walked or the result cannot be written.
fn run_files(command: FilesCommand) -> Result<ExitCode, Error> {
    match command {
        FilesCommand::Check { root, excludes } => {
            report(&conceptory_files::check(&root, &excludes)?)
        },
    }
}

/// Runs a `conceptory okf` subcommand.
///
/// # Errors
///
/// Returns an error if an OKF bundle or schema cannot be loaded, the result cannot be written,
/// or the viewer cannot be served.
fn run_okf(command: OkfCommand) -> Result<ExitCode, Error> {
    match command {
        OkfCommand::Check { roots, schemas } => {
            let schemas = Schemas::load(&schemas)?;
            let loaded = conceptory_okf::load(&roots)?;
            report(&conceptory_okf::check(&loaded, &schemas))
        },
        OkfCommand::Index { roots } => {
            let loaded = conceptory_okf::load(&roots)?;
            for (path, content) in conceptory_okf::indexes(&loaded.bundles) {
                write_file(&path, &content)?;
            }
            Ok(ExitCode::SUCCESS)
        },
        OkfCommand::Terms { roots, prh } => {
            let loaded = conceptory_okf::load(&roots)?;
            match prh {
                Some(path) => write_file(&path, &conceptory_okf::prh_dictionary(&loaded.bundles))?,
                None => write_stdout(&conceptory_okf::term_list(&loaded.bundles))?,
            }
            Ok(ExitCode::SUCCESS)
        },
        OkfCommand::Vocabulary { roots, out } => {
            let loaded = conceptory_okf::load(&roots)?;
            write_file(
                &out,
                &conceptory_okf::vocabulary_dictionary(&loaded.bundles),
            )?;
            Ok(ExitCode::SUCCESS)
        },
        OkfCommand::View {
            roots,
            schemas,
            port,
            no_open,
            allowed_origins,
        } => {
            conceptory_server::serve(conceptory_server::Config {
                roots,
                schemas,
                port,
                open: !no_open,
                allowed_origins,
            })?;
            Ok(ExitCode::SUCCESS)
        },
    }
}

/// Writes the diagnostics to stdout and their count to stderr. Returns a failure exit code if
/// there is at least one diagnostic.
///
/// # Errors
///
/// Returns [`Error::Io`] if the output cannot be written.
fn report(diagnostics: &[Diagnostic]) -> Result<ExitCode, Error> {
    for diagnostic in diagnostics {
        write_stdout(&format!("{diagnostic}\n"))?;
    }
    if diagnostics.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        write_stderr(&format!("{} 件の問題が見つかりました\n", diagnostics.len()))?;
        Ok(ExitCode::FAILURE)
    }
}

/// Writes a file after creating its parent directories.
///
/// # Errors
///
/// Returns [`Error::Io`] if a directory cannot be created or the file cannot be written.
fn write_file(path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes to stdout.
///
/// # Errors
///
/// Returns [`Error::Io`] if the write fails.
fn write_stdout(text: &str) -> Result<(), Error> {
    io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .map_err(|source| Error::Io {
            path: PathBuf::from("<stdout>"),
            source,
        })
}

/// Writes to stderr.
///
/// # Errors
///
/// Returns [`Error::Io`] if the write fails.
fn write_stderr(text: &str) -> Result<(), Error> {
    io::stderr()
        .lock()
        .write_all(text.as_bytes())
        .map_err(|source| Error::Io {
            path: PathBuf::from("<stderr>"),
            source,
        })
}
