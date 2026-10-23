use charon_lib::{
    export::CrateData, formatter::IntoFormatter, options::SerializationFormat, pretty::FmtWithCtx,
};
use std::{env, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 5 || args[1] != "--function" || args[3] != "--output" {
        return Err("usage: ullbc-pcg INPUT.ullbc --function NAME --output DIRECTORY".into());
    }
    let data = CrateData::deserialize_from_file(Path::new(&args[0]), SerializationFormat::Json)?;
    if data.has_errors {
        return Err("Charon reported extraction errors; refusing partial input".into());
    }
    let krate = &data.translated;
    let fmt = krate.into_fmt();
    let matches: Vec<_> = krate
        .fun_decls
        .iter()
        .filter(|f| {
            let name = f.item_meta.name.to_string_with_ctx(&fmt);
            name == args[2] || name.rsplit("::").next() == Some(args[2].as_str())
        })
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "expected one function matching {:?}, found {}",
            args[2],
            matches.len()
        )
        .into());
    }
    let output = ullbc_pcg::export::write(krate, matches[0], Path::new(&args[4]))?;
    println!(
        "Analyzed {}: {} ULLBC locations → {}/analysis.json",
        output.name,
        output.points.len(),
        args[4]
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("ullbc-pcg: {e}");
        std::process::exit(1);
    }
}
