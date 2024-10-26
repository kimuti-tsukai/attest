mod subcommands;
mod submit;
mod test;
mod utils;

use test::{test, Res};

use submit::submit;

use anyhow::Result;

use clap::{Parser, Subcommand};

/// Tester for AtCoder examples.
/// This tests your program in the example cases
#[derive(Parser)]
enum Arg {
    /// Test the examples
    #[clap(visible_alias("t"))]
    Test {
        #[arg(help = "URL of AtCoder")]
        url: Option<String>,

        #[clap(short = 'b', long = "build")]
        build: bool,

        #[clap(short = 'n', long = "num", num_args = 0.., value_delimiter = ' ')]
        example_num: Vec<usize>,
    },
    /// Submit your code
    #[clap(visible_alias("s"))]
    Submit {
        url: Option<String>,

        #[clap(short = 'l', long = "lang")]
        lang: Option<String>,
    },
    /// Test and Submit if all tests get AC
    #[clap(visible_alias("ts"))]
    Tebmit {
        url: Option<String>,

        #[clap(short = 'b', long = "build")]
        build: bool,

        #[clap(short = 'l', long = "lang")]
        lang: Option<String>,
    },
    /// Show or Select langs
    #[clap(visible_alias("l"))]
    Lang {
        lang: Option<String>,

        #[clap(short = 'l', long = "list")]
        list: bool,

        #[clap(short = 'u', long = "url")]
        url: Option<String>,

        #[clap(short = 's', long = "search")]
        search: Option<String>,
    },
    /// Init the environment to test
    Init,
    /// Set the environment to test
    Set {
        #[command(subcommand)]
        command: Option<Sets>,
    },
    /// Login to AtCoder
    Login {
        #[arg()]
        user_name: String,

        #[arg()]
        password: String,
    },
    /// Logout from AtCoder
    Logout,
}

#[derive(Subcommand)]
enum Sets {
    /// Set the build command
    Build {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    /// Set the run command
    Run {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    #[command(about = r#"Set the test command
Test command has to be satisfied with below:

You had better use command line arguments.

Output:
    ```
    J
    D
    ```
    J: If judge is correct `true` else `false`
    D (Option): Other discription

Input:
    ```
    R
    C
    ```
    R: Result the executing the run command
    C: Correct answer
    You can receive input as either stdin or command line arguments"#)]
    Test {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    /// Set the program file
    File { file_path: String },
    /// Set other files that building depends on
    Deps {
        #[clap(short = 'a', long = "add")]
        add: bool,
        #[clap(value_delimiter = ' ')]
        paths: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Arg = Arg::parse();

    match args {
        Arg::Test {
            url,
            example_num,
            build,
        } => {
            test(url, example_num, build).await?;
        }
        Arg::Submit { url, lang } => {
            submit(url, lang).await;
        }
        Arg::Tebmit { url, lang, build } => {
            let results: Option<Vec<Option<Res>>> = test(url.clone(), Vec::new(), build).await?;

            if let Some(v) = results {
                if v.iter().all(|&a: &Option<Res>| a == Some(Res::AC)) {
                    submit(url, lang).await;
                }
            }
        }
        Arg::Lang {
            lang,
            list,
            url,
            search,
        } => subcommands::lang(lang, list, url, search).await?,
        Arg::Init => subcommands::init(),
        Arg::Set { command } => {
            if let Some(command) = command {
                match command {
                    Sets::Build { command } => subcommands::set_build(command),
                    Sets::Run { command } => subcommands::set_run(command),
                    Sets::Test { command } => subcommands::set_test(command),
                    Sets::File { file_path } => subcommands::set_file(file_path),
                    Sets::Deps { paths, add } => subcommands::set_deps_file(paths, add),
                }
            } else {
                subcommands::show_set();
            }
        }
        Arg::Login {
            user_name,
            password,
        } => subcommands::login(user_name, password).await,
        Arg::Logout => subcommands::logout(),
    }

    Ok(())
}
