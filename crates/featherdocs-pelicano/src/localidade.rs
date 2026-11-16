//! Textos da interface em pt-BR, en-US e es-ES.
//!
//! O português é a fonte. [`tr`] aceita uma chave de `dados/pt-BR.json` ("salvar") ou o próprio
//! texto em português ("Pincel de formatação"); em inglês ou espanhol procura esse português em
//! `dados/en-US.json` / `dados/es-ES.json` e, se faltar, mostra o português. O idioma é global e a
//! tela troca na hora: as `view` chamam `tr` a cada quadro.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// Os idiomas da interface; o código é o que fica gravado na configuração.
pub const IDIOMAS: [(&str, &str); 3] = [("pt-BR", "Português (Brasil)"), ("en-US", "English"), ("es-ES", "Español")];

static LINGUA: AtomicU8 = AtomicU8::new(0);

fn indice(codigo: &str) -> u8 {
    if codigo.starts_with("en") {
        1
    } else if codigo.starts_with("es") {
        2
    } else {
        0
    }
}

/// O código suportado mais próximo do idioma do sistema ("en_GB.UTF-8" → "en-US").
pub fn suportado(sistema: Option<&str>) -> &'static str {
    IDIOMAS[usize::from(indice(sistema.unwrap_or("pt")))].0
}

/// Troca o idioma da interface (na abertura e nas Configurações).
pub fn definir(codigo: &str) {
    LINGUA.store(indice(codigo), Ordering::Relaxed);
    // O texto que não diz a língua dele é verificado na da interface.
    featherdocs_core::ortografia::idioma_do_app(codigo);
    // O F12 e a barra de baixo do feather-ui usam o mesmo dicionário.
    feather_ui::idioma::definir_tradutor(|pt| match LINGUA.load(Ordering::Relaxed) {
        0 => None,
        l => dicionario(l).get(pt).map(String::as_str),
    });
}

/// O código do idioma atual.
pub fn atual() -> &'static str {
    IDIOMAS[usize::from(LINGUA.load(Ordering::Relaxed))].0
}

fn ler(json: &str) -> HashMap<String, String> {
    let mapa: HashMap<String, String> = serde_json::from_str(json).expect("Localidade inválida");
    mapa.into_iter().filter(|(k, _)| !k.starts_with("//")).collect()
}

fn chaves() -> &'static HashMap<String, String> {
    static PT: OnceLock<HashMap<String, String>> = OnceLock::new();
    PT.get_or_init(|| ler(include_str!("../dados/pt-BR.json")))
}

/// Português → idioma `l` (1 = inglês, 2 = espanhol).
fn dicionario(l: u8) -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ES: OnceLock<HashMap<String, String>> = OnceLock::new();
    if l == 1 {
        EN.get_or_init(|| ler(include_str!("../dados/en-US.json")))
    } else {
        ES.get_or_init(|| ler(include_str!("../dados/es-ES.json")))
    }
}

/// O texto no idioma da interface: `chave` é uma chave de `pt-BR.json` ou o texto em português.
pub fn tr(chave: &str) -> &str {
    let pt = chaves().get(chave).map_or(chave, String::as_str);
    match LINGUA.load(Ordering::Relaxed) {
        0 => pt,
        l => dicionario(l).get(pt).map_or(pt, String::as_str),
    }
}

/// O texto em português `pt` no idioma `codigo`, sem trocar o da interface.
pub fn tr_em<'a>(codigo: &str, pt: &'a str) -> &'a str {
    match indice(codigo) {
        0 => pt,
        l => dicionario(l).get(pt).map_or(pt, String::as_str),
    }
}

/// Como [`tr`], com os `{}` do modelo trocados pelos valores, na ordem.
pub fn trf(modelo: &str, valores: &[&dyn std::fmt::Display]) -> String {
    let mut saida = String::new();
    let mut resto = tr(modelo);
    for v in valores {
        let Some(i) = resto.find("{}") else { break };
        saida.push_str(&resto[..i]);
        saida.push_str(&v.to_string());
        resto = &resto[i + 2..];
    }
    saida.push_str(resto);
    saida
}

/// Uma lista fixa de opções traduzida (as listas suspensas pedem `&'static [&'static str]`).
/// Cada lista é traduzida uma vez por idioma; a posição de cada opção não muda.
pub fn lista(pt: &'static [&'static str]) -> &'static [&'static str] {
    let l = LINGUA.load(Ordering::Relaxed);
    if l == 0 {
        return pt;
    }
    type Cache = Mutex<Vec<((usize, u8), &'static [&'static str])>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let chave = (pt.as_ptr() as usize, l);
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, lista)) = cache.iter().find(|(k, _)| *k == chave) {
        return lista;
    }
    let traduzida: &'static [&'static str] = Box::leak(pt.iter().map(|s| tr(s)).collect::<Vec<_>>().into_boxed_slice());
    cache.push((chave, traduzida));
    traduzida
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Todo texto de `pt-BR.json` e dos dicionários existe nos dois idiomas, com os mesmos `{}`
    /// (um marcador perdido mostraria o valor no lugar errado).
    #[test]
    fn dicionarios_cobrem_os_mesmos_textos_com_os_mesmos_marcadores() {
        let en = dicionario(1);
        let es = dicionario(2);
        let mut faltam = Vec::new();
        for pt in chaves().values().chain(en.keys()).chain(es.keys()) {
            for (nome, d) in [("en-US", en), ("es-ES", es)] {
                match d.get(pt) {
                    None => faltam.push(format!("{nome}: {pt:?}")),
                    Some(t) if t.matches("{}").count() != pt.matches("{}").count() => faltam.push(format!("{nome} (marcadores): {pt:?}")),
                    _ => {}
                }
            }
        }
        faltam.sort();
        faltam.dedup();
        assert!(faltam.is_empty(), "{} textos sem tradução:\n{}", faltam.len(), faltam.join("\n"));
    }

    /// Todo tr/trf com texto literal no código tem tradução (chave por variável é conferida à mão).
    #[test]
    fn todo_tr_literal_do_codigo_tem_traducao() {
        fn arquivos(pasta: &std::path::Path, saida: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(pasta).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { arquivos(&p, saida) } else if p.extension().is_some_and(|x| x == "rs") { saida.push(p) }
            }
        }
        let mut lista = Vec::new();
        arquivos(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut lista);
        let mut faltam = Vec::new();
        for f in lista {
            let codigo = std::fs::read_to_string(&f).unwrap();
            for (i, _) in codigo.match_indices("tr") {
                let antes = codigo[..i].chars().next_back();
                if antes.is_some_and(|c| c.is_alphanumeric() || c == '_') { continue; }
                let resto = codigo[i + 2..].strip_prefix('f').unwrap_or(&codigo[i + 2..]);
                let Some(resto) = resto.strip_prefix('(').map(str::trim_start) else { continue };
                // Texto direto ou `if x { texto } else { texto }`: os literais até o primeiro parêntese que fecha.
                let trecho = if resto.starts_with("if ") { &resto[..resto.find(')').unwrap_or(0)] } else if resto.starts_with('"') { resto } else { continue };
                let mut cs = trecho.chars();
                while cs.by_ref().any(|c| c == '"') {
                    let mut texto = String::new();
                    while let Some(c) = cs.next() {
                        match c {
                            '"' => break,
                            '\\' => match cs.next() { Some('n') => texto.push('\n'), Some(o) => texto.push(o), None => {} },
                            _ => texto.push(c),
                        }
                    }
                    let pt = chaves().get(&texto).cloned().unwrap_or(texto);
                    if !dicionario(1).contains_key(&pt) || !dicionario(2).contains_key(&pt) {
                        faltam.push(format!("{}: {pt:?}", f.file_name().unwrap().to_string_lossy()));
                    }
                    if !resto.starts_with("if ") { break; }
                }
            }
        }
        faltam.sort();
        faltam.dedup();
        assert!(faltam.is_empty(), "{} textos sem tradução:\n{}", faltam.len(), faltam.join("\n"));
    }

    /// Nomes e descrições dos modelos e os cabeçalhos da lista de recentes passam por `tr` de forma indireta.
    #[test]
    fn modelos_e_cabecalhos_da_tela_inicial_tem_traducao() {
        let textos = featherdocs_core::modelos::lista().iter().flat_map(|m| [m.nome, m.descricao]).chain(["Nome", "Pasta", "Modificado"]);
        let faltam: Vec<_> = textos.filter(|t| !dicionario(1).contains_key(*t) || !dicionario(2).contains_key(*t)).collect();
        assert!(faltam.is_empty(), "{faltam:?}");
    }

    #[test]
    fn idioma_do_sistema_cai_no_suportado() {
        assert_eq!(suportado(Some("en_GB.UTF-8")), "en-US");
        assert_eq!(suportado(Some("es-MX")), "es-ES");
        assert_eq!(suportado(Some("fr-FR")), "pt-BR");
        assert_eq!(suportado(None), "pt-BR");
    }
}
