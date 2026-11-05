//! Os modelos da pessoa (Arquivos → Salvar como modelo): um `.fdoz` cada, na pasta de dados do app,
//! listados na tela inicial antes dos que vêm com o FeatherDocs. Abrir um sempre cria um documento
//! novo, sem arquivo: salvar não grava por cima do modelo.
use featherdocs_core::modelo::Documento;
use std::path::{Path, PathBuf};

/// A pasta dos modelos: ao lado da de recuperação (`…/featherdocs/modelos`).
pub fn pasta() -> Option<PathBuf> {
    Some(crate::recuperacao::pasta()?.parent()?.join("modelos"))
}

/// Os modelos de `pasta`, pelo nome.
pub fn listar_em(pasta: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "fdoz"))
        .collect();
    v.sort_by_key(|p| nome(p).to_lowercase());
    v
}

/// O nome que a tela mostra: o do arquivo, sem a extensão.
pub fn nome(p: &Path) -> String {
    p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Grava `doc` como o modelo `nome` em `pasta` (sem histórico de versões: o modelo é ponto de
/// partida). O mesmo nome substitui o modelo anterior.
pub fn gravar_em(pasta: &Path, nome: &str, doc: &Documento) -> Result<PathBuf, String> {
    // O nome vira nome de arquivo: tira o que o Windows e o Linux não aceitam.
    let limpo: String = nome.trim().chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '-' } else { c }).take(80).collect();
    let limpo = limpo.trim().trim_matches('.').to_string();
    if limpo.is_empty() {
        return Err(crate::localidade::tr("Dê um nome ao modelo.").into());
    }
    std::fs::create_dir_all(pasta).map_err(|e| e.to_string())?;
    let p = pasta.join(format!("{limpo}.fdoz"));
    crate::arquivos::gravar(&p, &featherdocs_io::fdoz::gravar(doc)?)?;
    Ok(p)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_lista_e_le_o_modelo() {
        let pasta = std::env::temp_dir().join(format!("featherdocs-modelos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let mut s = featherdocs_core::sessao::Sessao::default();
        s.inserir_texto("Ofício nº");
        let p = gravar_em(&pasta, "  Ofício: semanal  ", s.documento()).unwrap();
        assert_eq!(nome(&p), "Ofício- semanal");
        assert!(gravar_em(&pasta, " .. ", s.documento()).is_err());
        assert_eq!(listar_em(&pasta), [p.clone()]);
        assert_eq!(crate::arquivos::ler(&p).unwrap().blocos, s.documento().blocos);
        std::fs::remove_dir_all(&pasta).unwrap();
    }
}
