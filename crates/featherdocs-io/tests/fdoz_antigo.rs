//! O `.fdoz` gravado antes da fase 0 da paridade (commit `7b1492a`) continua abrindo igual.
//! `dados/antes_fase0.fdoz` é a soma dos 209 `.docx` do exemplo `paridade_docx`, lidos e gravados
//! pelo código daquele commit; `dados/antes_fase0.json` é o documento que ele abria (o `serde`
//! do modelo). Não regravar: o arquivo vale como era.

use featherdocs_io::fdoz;

#[test]
fn le_o_fdoz_de_antes_da_fase0() {
    let dados = concat!(env!("CARGO_MANIFEST_DIR"), "/dados/");
    let bytes = std::fs::read(format!("{dados}antes_fase0.fdoz")).unwrap();
    let esperado = std::fs::read_to_string(format!("{dados}antes_fase0.json")).unwrap();
    let doc = fdoz::ler(&bytes).unwrap();
    assert!(serde_json::to_string_pretty(&doc).unwrap() == esperado, "o .fdoz antigo abriu diferente");
    // E grava e relê sem perder nada.
    assert_eq!(fdoz::ler(&fdoz::gravar(&doc).unwrap()).unwrap(), doc);
}
