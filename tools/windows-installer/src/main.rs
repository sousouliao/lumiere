#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}",
            serde_json::json!({"level":"error","event":"installer-transaction-failed","error":error.to_string()})
        );
        let _ = std::fs::write(
            std::env::temp_dir().join("Lumiere-installer-error.txt"),
            format!(
                "{error}\narguments: {:?}",
                std::env::args().skip(1).collect::<Vec<_>>()
            ),
        );
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next().ok_or("Missing command")?;
    let root = PathBuf::from(args.next().ok_or("Missing installation root")?);
    match command.to_str().ok_or("Invalid command")? {
        "begin" => {
            let owner = args
                .next()
                .ok_or("Missing installer PID")?
                .to_string_lossy()
                .parse()?;
            let shortcuts: Vec<_> = args.map(PathBuf::from).collect();
            lumiere_installer::begin(&root, owner, &shortcuts)?;
        }
        "commit" => {
            let manifest = args.next().ok_or("Missing payload manifest")?;
            let manifest = serde_json::from_slice(&std::fs::read(manifest)?)?;
            lumiere_installer::commit(&root, &manifest)?;
        }
        "rollback" => lumiere_installer::rollback(&root)?,
        "locate" => {
            if let Some(path) = lumiere_installer::inherited_root() {
                {
                    use std::os::windows::ffi::OsStrExt;
                    let bytes: Vec<u8> = path
                        .as_os_str()
                        .encode_wide()
                        .flat_map(u16::to_le_bytes)
                        .collect();
                    std::fs::write(root, bytes)?;
                }
            }
        }
        _ => return Err("Unknown command".into()),
    }
    Ok(())
}
