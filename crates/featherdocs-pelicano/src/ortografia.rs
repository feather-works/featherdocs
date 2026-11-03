//! O dicionário pt-BR (VERO, do LibreOffice) que vai dentro do programa: ver `dados/ortografia/LEIA-ME.md`.
use std::io::Read;

const DICIONARIO: &[u8] = include_bytes!("../dados/ortografia/pt_BR.zip");

/// Lê o dicionário embutido (leva um ou dois segundos: chamar fora da thread da tela).
pub fn carregar() -> Result<featherdocs_core::ortografia::Corretor, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(DICIONARIO)).map_err(|e| e.to_string())?;
    let mut ler = |nome: &str| -> Result<String, String> {
        let mut s = String::new();
        zip.by_name(nome).map_err(|e| e.to_string())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let (aff, dic) = (ler("pt_BR.aff")?, ler("pt_BR.dic")?);
    featherdocs_core::ortografia::Corretor::novo(&aff, &dic)
}

#[cfg(test)]
mod testes {
    #[test]
    fn o_dicionario_embutido_conhece_o_portugues() {
        let t = std::time::Instant::now();
        let c = super::carregar().unwrap();
        eprintln!("dicionário carregado em {:?}", t.elapsed());
        for certa in ["exceção", "Exceção", "guarda-chuva", "contribuíram", "paralelepípedo", "também"] {
            assert!(c.correta(certa), "{certa}");
        }
        for errada in ["excessão", "concerteza", "previlégio"] {
            assert!(!c.correta(errada), "{errada}");
        }
        assert!(c.sugestoes("excessão").iter().any(|s| s == "exceção"), "{:?}", c.sugestoes("excessão"));
    }
}
