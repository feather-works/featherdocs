//! Atalhos configuráveis (Configurações → Atalhos), como no FeatherSheet. A combinação escolhida
//! é traduzida para a de fábrica antes de chegar ao app e ao motor; a de fábrica trocada deixa de valer.
use crate::compat::keyboard::{Key, Modifiers};
use std::collections::BTreeMap;

/// (id guardado na configuração, rótulo, combinação de fábrica).
pub const ATALHOS: &[(&str, &str, &str)] = &[
    ("novo", "Novo documento", "Ctrl+N"),
    ("abrir", "Abrir", "Ctrl+O"),
    ("salvar", "Salvar", "Ctrl+S"),
    ("salvar_como", "Salvar como", "Ctrl+Shift+S"),
    ("imprimir", "Imprimir", "Ctrl+P"),
    ("pdf", "Exportar PDF", "Ctrl+Shift+P"),
    ("buscar", "Localizar", "Ctrl+F"),
    ("desfazer", "Desfazer", "Ctrl+Z"),
    ("refazer", "Refazer", "Ctrl+Y"),
    ("tudo", "Selecionar tudo", "Ctrl+A"),
    ("colar_sem", "Colar sem formatação", "Ctrl+Shift+V"),
    ("link", "Inserir link", "Ctrl+K"),
    ("negrito", "Negrito", "Ctrl+B"),
    ("italico", "Itálico", "Ctrl+I"),
    ("sublinhado", "Sublinhado", "Ctrl+U"),
    ("tachado", "Tachado", "Ctrl+Shift+X"),
    ("codigo", "Código", "Ctrl+Shift+E"),
    ("pincel", "Copiar formatação (pincel)", "Ctrl+Shift+C"),
    ("esquerda", "Alinhar à esquerda", "Ctrl+L"),
    ("centro", "Centralizar", "Ctrl+E"),
    ("direita", "Alinhar à direita", "Ctrl+R"),
    ("justificar", "Justificar", "Ctrl+J"),
    ("normal", "Texto normal", "Ctrl+Alt+0"),
    ("titulo1", "Título 1", "Ctrl+Alt+1"),
    ("titulo2", "Título 2", "Ctrl+Alt+2"),
    ("titulo3", "Título 3", "Ctrl+Alt+3"),
    ("menu", "Menu Arquivo", "Ctrl+,"),
];

/// A combinação em uso para `id`.
pub fn atual<'a>(escolhidos: &'a BTreeMap<String, String>, id: &str) -> &'a str {
    let padrao = ATALHOS.iter().find(|a| a.0 == id).map_or("", |a| a.2);
    escolhidos.get(id).map_or(padrao, String::as_str)
}

pub enum Traducao {
    /// Nenhum atalho trocado envolve a tecla: segue como veio.
    Igual,
    /// A combinação escolhida para um comando: vira a de fábrica dele.
    Trocar(Key, Modifiers),
    /// A de fábrica de um comando que ganhou outra combinação: não faz nada.
    Engolir,
}

pub fn traduzir(escolhidos: &BTreeMap<String, String>, tecla: &Key, m: Modifiers) -> Traducao {
    if escolhidos.is_empty() {
        return Traducao::Igual;
    }
    let Some(combo) = feather_ui::atalhos::combo_de(tecla, m) else { return Traducao::Igual };
    if let Some(&(_, _, padrao)) = ATALHOS.iter().find(|a| atual(escolhidos, a.0) == combo) {
        return if padrao == combo { Traducao::Igual } else { tecla_de(padrao).map_or(Traducao::Igual, |(k, m)| Traducao::Trocar(k, m)) };
    }
    if ATALHOS.iter().any(|a| a.2 == combo) {
        return Traducao::Engolir;
    }
    Traducao::Igual
}

/// "Ctrl+Shift+S" → a tecla `s` com Ctrl e Shift.
fn tecla_de(combo: &str) -> Option<(Key, Modifiers)> {
    let (mods, nome) = combo.rsplit_once('+').filter(|(_, n)| !n.is_empty()).unwrap_or(("", combo));
    let mut m = Modifiers::empty();
    for p in mods.split('+') {
        match p {
            "Ctrl" => m |= Modifiers::CONTROL,
            "Alt" => m |= Modifiers::ALT,
            "Shift" => m |= Modifiers::SHIFT,
            _ => {}
        }
    }
    (nome.chars().count() == 1).then(|| (Key::Character(nome.to_lowercase().into()), m))
}

/// Reservadas: copiar, recortar, colar, Esc e as da suíte (F1, F12, Ctrl+F1).
pub fn valido(escolhidos: &BTreeMap<String, String>, id: &str, combo: &str) -> Result<(), &'static str> {
    if !(combo.starts_with("Ctrl+") || combo.starts_with("Alt+")) || combo.len() > 40 {
        return Err("Escolha um atalho com Ctrl ou Alt.");
    }
    if matches!(combo, "Ctrl+C" | "Ctrl+X" | "Ctrl+V" | "Ctrl+F1") {
        return Err("Combinação reservada (copiar, recortar, colar ou faixa).");
    }
    if ATALHOS.iter().any(|a| a.0 != id && atual(escolhidos, a.0) == combo) {
        return Err("Este atalho já está em uso.");
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn atalho_trocado_vira_o_de_fabrica_e_o_antigo_para() {
        let mut e = BTreeMap::new();
        e.insert("negrito".to_string(), "Ctrl+Shift+N".to_string());
        assert!(valido(&e, "salvar", "Ctrl+Shift+N").is_err(), "já em uso");
        assert!(valido(&e, "salvar", "Ctrl+V").is_err());
        assert!(valido(&e, "salvar", "Ctrl+Shift+D").is_ok());
        let k = |c: &str| Key::Character(c.into());
        match traduzir(&e, &k("n"), Modifiers::CONTROL | Modifiers::SHIFT) {
            Traducao::Trocar(Key::Character(c), m) => assert!(c.as_str() == "b" && m == Modifiers::CONTROL),
            _ => panic!("Ctrl+Shift+N devia virar Ctrl+B"),
        }
        assert!(matches!(traduzir(&e, &k("b"), Modifiers::CONTROL), Traducao::Engolir));
        assert!(matches!(traduzir(&e, &k("i"), Modifiers::CONTROL), Traducao::Igual));
    }
}
