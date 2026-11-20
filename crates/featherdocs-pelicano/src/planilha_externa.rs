//! Abrir dados no FeatherSheet instalado: o contrato é um CSV (`featherdocs_io::csv`), nenhum app
//! importa código do outro.
use std::path::PathBuf;

use crate::localidade::{tr, trf};

/// O FeatherSheet instalado: no Windows, a pasta do instalador; senão, pelo PATH (deb, ~/.local/bin, snap).
fn programa() -> Option<PathBuf> {
    let pastas = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    let nome = if cfg!(windows) { "feathersheet.exe" } else { "feathersheet" };
    let instalado = std::env::var_os("ProgramW6432").map(|p| PathBuf::from(p).join("FeatherWorks").join("FeatherSheet"));
    instalado.into_iter().chain(pastas).map(|p| p.join(nome)).find(|p| p.is_file())
}

/// Grava o CSV e abre no FeatherSheet. O snap tem `/tmp` próprio e não lê pasta oculta: para ele o
/// arquivo vai para `~/FeatherDocs`.
pub fn abrir(csv: &str) -> Result<(), String> {
    let programa = programa().ok_or_else(|| tr("O FeatherSheet não foi encontrado. Instale pelo site da FeatherWorks.").to_owned())?;
    let pasta = match std::env::var_os("HOME") {
        Some(h) if programa.starts_with("/snap") => PathBuf::from(h).join("FeatherDocs"),
        _ => std::env::temp_dir().join("featherdocs"),
    };
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    let segundos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let arquivo = pasta.join(format!("tabela-{segundos}.csv"));
    // BOM: acentos lidos como UTF-8 também no Excel, se o CSV for parar lá.
    std::fs::write(&arquivo, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    std::process::Command::new(&programa).arg(&arquivo).spawn().map(drop).map_err(|e| trf("Não foi possível abrir o FeatherSheet: {}", &[&e.to_string()]))
}
