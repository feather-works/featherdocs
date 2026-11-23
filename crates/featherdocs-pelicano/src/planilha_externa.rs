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
        Some(h) if programa.starts_with("/snap") => {
            let p = PathBuf::from(h).join("FeatherDocs");
            std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
            p
        }
        _ => pasta_temporaria().map_err(|e| e.to_string())?,
    };
    // Um CSV por clique: os de antes já foram abertos (ou esquecidos) e não ficam acumulando.
    for e in std::fs::read_dir(&pasta).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with("tabela-") && n.ends_with(".csv") { let _ = std::fs::remove_file(e.path()); }
    }
    let segundos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let arquivo = pasta.join(format!("tabela-{segundos}.csv"));
    // BOM: acentos lidos como UTF-8 também no Excel, se o CSV for parar lá.
    std::fs::write(&arquivo, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    std::process::Command::new(&programa).arg(&arquivo).spawn().map(drop).map_err(|e| trf("Não foi possível abrir o FeatherSheet: {}", &[&e.to_string()]))
}

/// Pasta dos arquivos de passagem (tabela para o FeatherSheet, PDF da impressão), só do usuário:
/// o /tmp é de todos do computador, e esses arquivos levam o conteúdo do documento.
pub(crate) fn pasta_temporaria() -> std::io::Result<PathBuf> {
    let pasta = crate::recuperacao::pasta()
        .and_then(|p| p.parent().map(|p| p.join("temporarios")))
        .unwrap_or_else(|| std::env::temp_dir().join("featherdocs"));
    std::fs::create_dir_all(&pasta)?;
    // Falha (e nada é gravado) se a pasta for de outro usuário.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pasta, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(pasta)
}

#[cfg(test)]
mod testes {
    #[cfg(unix)]
    #[test]
    fn pasta_temporaria_e_so_do_usuario() {
        use std::os::unix::fs::PermissionsExt;
        let p = super::pasta_temporaria().unwrap();
        assert!(!p.starts_with(std::env::temp_dir()) || std::env::var_os("HOME").is_none());
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o700);
    }
}
