use clap::Clap;
use image::io::Reader as ImageReader;
use pmd_cte::{CteFormat, CteImage};
use std::{fs::File, io::BufReader, path::PathBuf};

/// ctetool can be used to encode or decode cte file (extension .img) from pokemon super mystery dungeon. It only support the font cte file.
#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Extract a cte file to an image
    Extract(ExtractParameter),
    /// Encode an image to a cte file
    Encode(EncodeParameter),
}

#[derive(Clap)]
struct ExtractParameter {
    /// the input .img cte file
    input: PathBuf,
    /// the output file (format determined by extension, .png recommanded)
    output: PathBuf,
}

#[derive(Clap)]
struct EncodeParameter {
    /// the input picture file
    input: PathBuf,
    /// the output .img cte file
    output: PathBuf,
}

fn main() {
    let opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Extract(ep) => extract(ep),
        SubCommand::Encode(ep) => encode(ep),
    };
}

fn extract(param: ExtractParameter) {
    println!(
        "extracting the file {:?} to {:?}",
        param.input, param.output
    );
    let mut in_file = BufReader::new(File::open(&param.input).unwrap());
    let cte_image = CteImage::decode_cte(&mut in_file).unwrap();
    cte_image.image.into_rgba8().save(&param.output).unwrap();
    println!("done !");
}

fn encode(param: EncodeParameter) {
    println!(
        "encoding {:?} into {:?} (using the A8 encoding)",
        param.input, param.output
    );
    let cte_image = CteImage {
        original_format: CteFormat::A8,
        image: ImageReader::open(&param.input).unwrap().decode().unwrap(),
    };
    let mut output = File::create(&param.output).unwrap();
    cte_image.encode_cte(&mut output).unwrap();
    println!("done");
}
