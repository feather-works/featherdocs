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
    let (pasta, guardar) = match std::env::var_os("HOME") {
        Some(h) if programa.starts_with("/snap") => {
            let p = PathBuf::from(h).join("FeatherDocs");
            std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
            // Pasta visível: a pessoa pode ter editado e salvo a tabela ali; só some depois de um mês.
            (p, 30 * 24 * 3600)
        }
        _ => (pasta_temporaria().map_err(|e| e.to_string())?, 3600),
    };
    // Os CSVs de antes não acumulam, mas só os velhos: o de um clique de agora (desta ou de outra
    // janela) o FeatherSheet ainda vai ler.
    apagar_antigos(&pasta, "tabela-", ".csv", guardar);
    let quando = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let arquivo = pasta.join(format!("tabela-{quando}-{}.csv", std::process::id()));
    // BOM: acentos lidos como UTF-8 também no Excel, se o CSV for parar lá.
    std::fs::write(&arquivo, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    std::process::Command::new(&programa).arg(&arquivo).spawn().map(drop).map_err(|e| trf("Não foi possível abrir o FeatherSheet: {}", &[&e.to_string()]))
}

/// Apaga em `pasta` os arquivos `prefixo…sufixo` com mais de `segundos` de idade.
pub(crate) fn apagar_antigos(pasta: &std::path::Path, prefixo: &str, sufixo: &str, segundos: u64) {
    let agora = std::time::SystemTime::now();
    for e in std::fs::read_dir(pasta).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let velho = e.metadata().and_then(|m| m.modified()).ok()
            .and_then(|m| agora.duration_since(m).ok()).is_some_and(|d| d.as_secs() > segundos);
        if n.starts_with(prefixo) && n.ends_with(sufixo) && velho { let _ = std::fs::remove_file(e.path()); }
    }
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
    #[test]
    fn so_o_arquivo_velho_sai() {
        let pasta = std::env::temp_dir().join(format!("featherdocs-antigos-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let novo = pasta.join("tabela-1.csv");
        std::fs::write(&novo, "a").unwrap();
        let outro = pasta.join("precos.csv");
        std::fs::write(&outro, "a").unwrap();
        super::apagar_antigos(&pasta, "tabela-", ".csv", 3600);
        assert!(novo.exists(), "o de agora fica");
        super::apagar_antigos(&pasta, "tabela-", ".csv", 0);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        super::apagar_antigos(&pasta, "tabela-", ".csv", 0);
        assert!(!novo.exists() && outro.exists(), "o velho sai; o que não é nosso fica");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[cfg(unix)]
    #[test]
    fn pasta_temporaria_e_so_do_usuario() {
        use std::os::unix::fs::PermissionsExt;
        let p = super::pasta_temporaria().unwrap();
        assert!(!p.starts_with(std::env::temp_dir()) || std::env::var_os("HOME").is_none());
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o700);
    }
}
