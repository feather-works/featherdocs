//! Os sinônimos do botão direito, do dicionário do LibreOffice que vai dentro do programa (ver
//! `dados/ortografia/LEIA-ME.md`). Carrega uma vez, fora da tela, junto com a ortografia.
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

const DADOS: &[u8] = include_bytes!("../dados/ortografia/sinonimos_pt_BR.zip");

/// Quantos sinônimos o menu mostra.
const NO_MENU: usize = 8;

static TABELA: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// Lê a tabela embutida (alguns décimos de segundo: chamar fora da thread da tela).
pub fn carregar() -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(DADOS)).map_err(|e| e.to_string())?;
    let mut texto = String::new();
    zip.by_name("sinonimos_pt_BR.txt").map_err(|e| e.to_string())?.read_to_string(&mut texto).map_err(|e| e.to_string())?;
    let tabela = texto
        .lines()
        .filter_map(|l| {
            let mut partes = l.split('|');
            let palavra = partes.next()?.to_string();
            Some((palavra, partes.map(String::from).collect()))
        })
        .collect();
    let _ = TABELA.set(tabela);
    Ok(())
}

/// Os sinônimos de `palavra` (vazio se a tabela ainda não chegou ou não conhece a palavra).
pub fn de(palavra: &str) -> Vec<String> {
    let Some(t) = TABELA.get() else { return Vec::new() };
    t.get(palavra).or_else(|| t.get(&palavra.to_lowercase())).map(|v| v.iter().take(NO_MENU).cloned().collect()).unwrap_or_default()
}

#[cfg(test)]
mod testes {
    #[test]
    fn a_tabela_embutida_conhece_o_portugues() {
        super::carregar().unwrap();
        let s = super::de("Bonito");
        assert!(s.iter().any(|x| x == "elegante") && s.len() <= super::NO_MENU, "{s:?}");
        assert!(super::de("xyzw").is_empty());
    }
}
