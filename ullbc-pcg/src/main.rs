use charon_lib::{
    ast::Body, export::CrateData, formatter::IntoFormatter, options::SerializationFormat,
    pretty::FmtWithCtx,
};
use std::{env, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let usage = "usage: ullbc-pcg INPUT.ullbc [--function NAME] --output DIRECTORY";
    let (input, options) = args.split_first().ok_or(usage)?;
    let (mut function, mut output) = (None, None);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        let value = options.next().ok_or(usage)?;
        match option.as_str() {
            "--function" if function.is_none() => function = Some(value),
            "--output" if output.is_none() => output = Some(value),
            _ => return Err(usage.into()),
        }
    }
    let output_dir = output.ok_or(usage)?;
    let data = CrateData::deserialize_from_file(Path::new(input), SerializationFormat::Json)?;
    if data.has_errors {
        return Err("Charon reported extraction errors; refusing partial input".into());
    }
    let krate = &data.translated;
    let fmt = krate.into_fmt();
    let matches: Vec<_> = krate
        .fun_decls
        .iter()
        .filter(|f| matches!(&f.body, Body::Unstructured(_)))
        .filter(|f| {
            let name = f.item_meta.name.to_string_with_ctx(&fmt);
            function.is_none_or(|wanted| {
                name == *wanted || name.rsplit("::").next() == Some(wanted.as_str())
            })
        })
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "expected one function matching {:?}, found {}. Use --function NAME. Available functions: {}",
            function, matches.len(),
            krate.fun_decls.iter().filter(|f| matches!(&f.body, Body::Unstructured(_)))
                .map(|f| f.item_meta.name.to_string_with_ctx(&fmt)).collect::<Vec<_>>().join(", ")
        )
        .into());
    }
    let output = ullbc_pcg::export::write(krate, matches[0], Path::new(output_dir))?;
    println!(
        "Analyzed {}: {} ULLBC locations → {}/analysis.json",
        output.name,
        output.points.len(),
        output_dir
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("ullbc-pcg: {e}");
        std::process::exit(1);
    }
}
