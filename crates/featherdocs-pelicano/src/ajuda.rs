//! Ajuda FeatherDocs portada de src/ajuda/{conteudo,secoes,pagina}.ts.
//! Textos originais vêm do JSON de idioma existente; zero conteúdo HTML.
use feather_ui::ajuda::{Catalogo, Linha, Secao, Topico};
use serde_json::Value;
use std::sync::OnceLock;
const JSON_PT: &str = include_str!("../dados/ajuda/pt-BR.json");
const JSON_EN: &str = include_str!("../dados/ajuda/en-US.json");
const JSON_ES: &str = include_str!("../dados/ajuda/es-ES.json");
const TOPICOS: &[&str] = &[
    "comecar", "formatar", "estrutura", "tabelas", "pagina", "arquivos", "configuracoes",
];
const GRUPOS: &[(&str, &[(&str, &str)])] = &[
    ("movimento", &[
        ("setas","← ↑ → ↓"), ("palavra","Ctrl + ← →"),
        ("linha","Home / End"), ("documento","Ctrl+Home / Ctrl+End"),
        ("pagina","PgUp / PgDn"), ("selecionar","Shift + qualquer um acima"),
    ]),
    ("edicao", &[
        ("desfazer","Ctrl+Z"), ("refazer","Ctrl+Y / Ctrl+Shift+Z"),
        ("selTudo","Ctrl+A"), ("area","Ctrl+C / Ctrl+X / Ctrl+V"),
        ("colarSem","Ctrl+Shift+V"), ("apagar","Backspace / Delete"),
        ("enter","Enter"), ("linhaQuebra","Shift+Enter"), ("tab","Tab / Shift+Tab"),
        ("quebra","Ctrl+Enter"), ("esc","Esc"),
    ]),
    ("formato", &[
        ("negrito","Ctrl+B"), ("italico","Ctrl+I"), ("sublinhado","Ctrl+U"),
        ("codigo","Ctrl+E"), ("tachado","Ctrl+Shift+X"),
        ("lista","Ctrl+Shift+8"), ("listaNum","Ctrl+Shift+7"),
        ("titulos","Ctrl+Alt+0 … 6"),
    ]),
    ("arquivo", &[
        ("novo","Ctrl+N"), ("abrir","Ctrl+O"),
        ("salvar","Ctrl+S"), ("salvarComo","Ctrl+Shift+S"),
        ("exportarPdf","Ctrl+P"),
    ]),
    ("janela", &[("ajuda","F1"), ("fechar","Esc")]),
];
fn caminho<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    let mut v=root;
    for k in key.split('.') { v=v.get(k)?; }
    Some(v)
}
fn tr(root: &Value, key: &str) -> String {
    caminho(root,key).and_then(Value::as_str).unwrap_or(key).to_owned()
}
fn carregar(idioma: &str) -> Catalogo {
    let origem = if idioma.starts_with("en") { JSON_EN } else if idioma.starts_with("es") { JSON_ES } else { JSON_PT };
    let json: Value=serde_json::from_str(origem).expect("Localidade de Ajuda inválida");
    let uso=TOPICOS.iter().map(|id| {
        let raiz=format!("ajuda.comoUsar.{id}");
        let linhas=caminho(&json,&format!("{raiz}.corpo"))
            .and_then(Value::as_array).into_iter().flatten()
            .filter_map(Value::as_str)
            .map(|s| match s.strip_prefix("• ") {
                Some(t)=>Linha::Marcador(t.to_owned()),
                None=>Linha::Texto(s.to_owned()),
            }).collect();
        Topico {id:(*id).into(),titulo:tr(&json,&format!("{raiz}.titulo")),linhas}
    }).collect();
    let atalhos=GRUPOS.iter().map(|(id, atalhos)| Topico {
        id:(*id).into(),
        titulo:tr(&json,&format!("ajuda.rotulos.g_{id}")),
        linhas:atalhos.iter().map(|(id,tecla)| Linha::Atalho {
            descricao:tr(&json,&format!("ajuda.teclas.{id}")),tecla:(*tecla).into(),
        }).collect(),
    }).collect();
    Catalogo {
        titulo:tr(&json,"ajuda.rotulos.titulo"),
        busca_rotulo:tr(&json,"ajuda.rotulos.busca"),
        nada_encontrado:tr(&json,"ajuda.rotulos.semResultado"),
        anterior:tr(&json,"ajuda.rotulos.anterior"),
        proximo:tr(&json,"ajuda.rotulos.proximo"),
        nesta_secao:tr(&json,"ajuda.rotulos.nestaSecao"),
        secoes:vec![
            Secao{id:"uso".into(),titulo:tr(&json,"ajuda.rotulos.uso"),
                introducao:tr(&json,"ajuda.introducoes.uso"),topicos:uso},
            Secao{id:"atalhos".into(),titulo:tr(&json,"ajuda.rotulos.atalhos"),
                introducao:tr(&json,"ajuda.introducoes.atalhos"),topicos:atalhos},
        ],
    }
}
/// A Ajuda no idioma atual da interface (cada idioma é montado uma vez).
pub fn modelo() -> &'static Catalogo {
    static MODELOS: [OnceLock<Catalogo>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let idioma = crate::localidade::atual();
    let i = crate::localidade::IDIOMAS.iter().position(|(c, _)| *c == idioma).unwrap_or(0);
    MODELOS[i].get_or_init(|| carregar(idioma))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn idiomas_originais_têm_mesma_cobertura() {
        for idioma in ["pt-BR","en-US","es-ES"] {
            let c=carregar(idioma);
            assert_eq!(c.secoes[0].topicos.len(),7);
            assert_eq!(c.secoes[1].topicos.len(),5);
            assert!(!c.titulo.trim().is_empty());
        }
    }
    #[test] fn portou_todos_os_topicos_e_atalhos_do_typescript() {
        let c=carregar("pt-BR");
        assert_eq!(c.secoes.len(),2);
        assert_eq!(c.secoes[0].topicos.len(),7);
        assert_eq!(c.secoes[1].topicos.len(),5);
        assert_eq!(c.secoes[1].topicos.iter().map(|t|t.linhas.len()).sum::<usize>(),32);
        assert!(c.quantidade_encontrada("formatação")>0);
        assert!(c.quantidade_encontrada("Ctrl+Shift+V")>0);
        assert!(c.quantidade_encontrada("Ctrl+P")>0);
        for s in &c.secoes {
            for t in &s.topicos { assert!(!t.titulo.contains("ajuda.")); }
        }
    }
}
