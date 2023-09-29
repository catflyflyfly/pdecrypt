use clap::Parser;
use clap::Subcommand;
use eyre::Result;

const DEFAULT_PW_LIST: &str = "~/.pdecrypt/pw_list.toml";

/// Decrypt all pdf files in a directory using a password list
///
/// To begin, run `pdecrypt init dd/mm/yyyy`
/// to configure the password list based on your date of birth.
///
/// Then, you can run `pdecrypt decrypt -i /path/to/pdfs/dir`
/// to generate a new directory with decrypted pdf files.
///
/// Works on both date format in CE (e.g. 2023) and in BE (e.g. 2566)
///
/// For more information, see GITHUB_HERE
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, default_value_t = false)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize password list comprises various formats of date of birth to default location
    Init(init::InitArgs),
    /// Decrypt all pdf files in a directory
    Decrypt(decrypt::DecryptArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => init::init(args, cli.verbose),
        Commands::Decrypt(args) => decrypt::decrypt(args, cli.verbose),
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct PasswordList {
    pw_list: Vec<String>,
}

mod decrypt {
    use std::env;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use chrono::Local;
    use clap::Args;
    use eyre::eyre;
    use eyre::Result;
    use itertools::Itertools;
    use qpdf::QPdf;

    use crate::PasswordList;
    use crate::DEFAULT_PW_LIST;

    #[derive(Debug, Args)]
    pub struct DecryptArgs {
        /// [default: pwd]
        #[arg(short, long)]
        input_dir: Option<PathBuf>,

        /// [default: [OUTPUT_DIR]_decrypted_[RANDOM_UUID_V4]]
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Password list file
        #[arg(short, long, default_value = DEFAULT_PW_LIST)]
        pw_list: String,
    }

    impl DecryptArgs {
        pub fn pw_list_file(&self) -> PathBuf {
            let expanded_pw_list = shellexpand::full(&self.pw_list).unwrap();

            Path::new(&expanded_pw_list.to_string()).to_path_buf()
        }
    }

    pub fn decrypt(args: DecryptArgs, verbose: bool) -> Result<()> {
        let pw_list = fs::read_to_string(args.pw_list_file())?;

        let PasswordList { pw_list } = toml::from_str(&pw_list)?;

        let input_dir = match args.input_dir {
            Some(dir) => dir,
            None => env::current_dir()?,
        };

        if verbose {
            println!("pdecrypt: Input directory: {}", input_dir.display());
        }

        let output_dir = match args.output_dir {
            Some(dir) => dir,
            None => create_output_dir(&input_dir, verbose)?,
        };

        if verbose {
            println!("pdecrypt: Output directory: {}", output_dir.display());
        }

        let (_, errors): (Vec<_>, Vec<_>) = pdf_files(&input_dir)?
            .iter()
            .map(|path| {
                let pdf = try_decrypt_from_password_list(path, &pw_list, verbose)?;

                let mut new_path = output_dir.clone();
                new_path.push(path.file_name().unwrap());

                if verbose {
                    println!("pdecrypt: Writing decrypted file: {:?}", new_path);
                }
                pdf.writer().preserve_encryption(false).write(&new_path)?;

                Ok::<_, eyre::Error>(pdf)
            })
            .partition_result();

        if !errors.is_empty() {
            if verbose {
                println!("pdecrypt: Failed to decrypt files:");
            }
            for err in errors {
                if verbose {
                    println!("{}", err);
                }
            }
        }

        if verbose {
            println!("pdecrypt: Done!");
        }

        Ok(())
    }

    fn create_output_dir(input_dir: &PathBuf, verbose: bool) -> Result<PathBuf> {
        let mut dirname = input_dir
            .file_name()
            .unwrap_or(OsStr::new(""))
            .to_os_string();

        dirname.push(format!(
            "_pdfs_decrypted_{}",
            Local::now().format("%Y%m%d%H%M%S")
        ));

        let mut output_dir = input_dir.clone();

        output_dir.pop();
        output_dir.push(dirname);

        if output_dir.exists() {
            return Err(eyre!(
                "pdecrypt: Output directory already exists: {}",
                output_dir.display()
            ));
        }

        if verbose {
            println!(
                "pdecrypt: Creating output directory: {}",
                output_dir.display()
            );
        }
        fs::create_dir(&output_dir)?;

        Ok(output_dir)
    }

    pub fn pdf_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
        let file_names = fs::read_dir(dir)?
            .filter_map(|f| f.ok())
            .map(|f| f.path())
            .filter(|path| {
                path.extension()
                    .map(|ext| ext.to_ascii_lowercase() == "pdf")
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        Ok(file_names)
    }

    pub fn try_decrypt_from_password_list(
        path: &PathBuf,
        password_list: &[String],
        verbose: bool,
    ) -> Result<QPdf> {
        if verbose {
            println!("pdecrypt: Trying to decrypt file: {:?}", path);
        }

        let Some(password) = password_list
            .iter()
            .find(|pw| QPdf::read_encrypted(path, pw).is_ok()) else {
                return Err(eyre!("pdecrypt: Failed to find password for file: {}", path.display()))
            };

        if verbose {
            println!("pdecrypt: Decrypting file with password: {}", password);
        }

        Ok(QPdf::read_encrypted(path, password)?)
    }
}

mod init {
    use std::fs;
    use std::io::Write;
    use std::iter::once;
    use std::path::Path;

    use chrono::NaiveDate;
    use clap::Args;
    use eyre::Result;
    use itertools::Itertools;

    use crate::PasswordList;
    use crate::DEFAULT_PW_LIST;

    #[derive(Debug, Args)]
    pub struct InitArgs {
        /// Date of birth. ex. 01/01/1999, 27/12/1994
        #[arg(value_parser = dob::parse_naive_date)]
        dob: NaiveDate,

        #[arg(value_parser = thai_citizen_id::parse_thai_citizen_id)]
        thai_citizen_id: String,
    }

    pub fn init(args: InitArgs, verbose: bool) -> Result<()> {
        let pw_list = PasswordList {
            pw_list: once(args.thai_citizen_id)
                .chain(dob::generate_formats(args.dob))
                .collect_vec(),
        };

        let dir = shellexpand::full("~/.pdecrypt/")?;

        if Path::new(&dir.to_string()).exists() {
            if verbose {
                println!("pdecrypt: Directory already exists: {}", dir);
            }
        } else {
            if verbose {
                println!("pdecrypt: Creating directory: {}", dir);
            }
            fs::create_dir(dir.to_string())?;
        }

        let pw_list_file = shellexpand::full(DEFAULT_PW_LIST)?;

        if verbose {
            println!("pdecrypt: Creating file: {}", pw_list_file);
        }
        let mut file = fs::File::create(pw_list_file.to_string())?;

        let toml = toml::to_string_pretty(&pw_list)?;

        file.write_all(toml.as_bytes())?;
        if verbose {
            println!("pdecrypt: Writing default password list");
        }

        file.sync_all()?;
        if verbose {
            println!("pdecrypt: Init done!");
        }

        Ok(())
    }

    mod dob {
        use chrono::NaiveDate;
        use chronoutil::RelativeDuration;

        pub fn parse_naive_date(s: &str) -> Result<NaiveDate, String> {
            NaiveDate::parse_from_str(s, "%d/%m/%Y").map_err(|e| e.to_string())
        }

        pub fn generate_formats(dob: NaiveDate) -> Vec<String> {
            let dobs = vec![dob + RelativeDuration::years(543), dob];

            let formats = vec![
                "%d%m%Y", // ex. 27121996
                "%d%m%y", // ex. 271296
                "%d%b%Y", // ex. 22Dec1996
                "%d%b%y", // ex. 22Dec96
            ];

            dobs.into_iter()
                .flat_map(|dob| {
                    formats
                        .iter()
                        .map(move |format| dob.format(format).to_string())
                })
                .collect()
        }
    }

    mod thai_citizen_id {
        pub fn parse_thai_citizen_id(s: &str) -> Result<String, String> {
            if s.len() != 13 {
                return Err("Invalid length".to_string());
            }

            if !s.chars().all(|c| c.is_ascii_digit()) {
                return Err("Invalid character".to_string());
            }

            Ok(s.to_string())
        }
    }
}
