//! Os dicionários (do LibreOffice) que vão dentro do programa: português (VERO), inglês (SCOWL) e
//! espanhol (RLA-ES). Ver `dados/ortografia/LEIA-ME.md` para a origem e a licença de cada um.
use std::io::Read;

/// O prefixo da língua, o nome dos arquivos `.aff`/`.dic` no zip e o zip.
const DICIONARIOS: [(&str, &str, &[u8]); 3] = [
    ("pt", "pt_BR", include_bytes!("../dados/ortografia/pt_BR.zip")),
    ("en", "en_US", include_bytes!("../dados/ortografia/en_US.zip")),
    ("es", "es_ES", include_bytes!("../dados/ortografia/es_ES.zip")),
];

/// Lê o dicionário embutido de nome `nome` (`pt_BR`): leva até um ou dois segundos, chamar fora da thread da tela.
pub fn carregar_idioma(nome: &str) -> Result<featherdocs_core::ortografia::Corretor, String> {
    let (_, _, bytes) = DICIONARIOS.iter().find(|(_, n, _)| *n == nome).ok_or_else(|| format!("Sem dicionário {nome}"))?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(*bytes)).map_err(|e| e.to_string())?;
    let mut ler = |arquivo: &str| -> Result<String, String> {
        let mut s = String::new();
        zip.by_name(arquivo).map_err(|e| e.to_string())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let (aff, dic) = (ler(&format!("{nome}.aff"))?, ler(&format!("{nome}.dic"))?);
    featherdocs_core::ortografia::Corretor::novo(&aff, &dic)
}

/// Carrega e instala os três dicionários, o português primeiro. O que falhar fica de fora (a língua
/// dele só não é verificada); o erro do primeiro que falhou volta para a barra de status.
pub fn carregar_todos() -> Result<(), String> {
    let mut erro = None;
    for (prefixo, nome, _) in DICIONARIOS {
        match carregar_idioma(nome) {
            Ok(c) => featherdocs_core::ortografia::instalar_idioma(prefixo, c),
            Err(e) => erro = erro.or(Some(e)),
        }
    }
    erro.map_or(Ok(()), Err)
}

#[cfg(test)]
mod testes {
    #[test]
    fn os_dicionarios_embutidos_conhecem_cada_lingua() {
        let t = std::time::Instant::now();
        let c = super::carregar_idioma("pt_BR").unwrap();
        eprintln!("dicionário pt carregado em {:?}", t.elapsed());
        for certa in ["exceção", "Exceção", "guarda-chuva", "contribuíram", "paralelepípedo", "também"] {
            assert!(c.correta(certa), "{certa}");
        }
        for errada in ["excessão", "concerteza", "previlégio"] {
            assert!(!c.correta(errada), "{errada}");
        }
        assert!(c.sugestoes("excessão").iter().any(|s| s == "exceção"), "{:?}", c.sugestoes("excessão"));
        let en = super::carregar_idioma("en_US").unwrap();
        assert!(en.correta("receive") && !en.correta("recieve"));
        assert!(en.sugestoes("recieve").iter().any(|s| s == "receive"));
        let es = super::carregar_idioma("es_ES").unwrap();
        assert!(es.correta("también") && es.correta("niño") && !es.correta("tambien"));
    }
}
