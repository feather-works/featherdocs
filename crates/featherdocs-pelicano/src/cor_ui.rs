//! As cores da faixa no jeito do Word: o ícone do que se pinta e, ao lado, a amostra com a cor
//! atual que abre o seletor do feather-ui (o mesmo do FeatherSheet) num menu da faixa
//! (`MenuFaixa::Cor`). A mudança vira `Msg::CorEscolhida`.
use crate::app::{App, Msg};
use crate::faixa::MenuFaixa;
use feather_ui::{icones::icone, tema::{estilo_botao_pequeno, COR_TEXTO_MUTED}};
use pelicano::{button, container, row, text, Align, Color, Element};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlvoCor {
    Texto,
    Realce,
    Fundo,
    Pagina,
    /// O sombreamento das células escolhidas de uma tabela.
    Celula,
}

/// A cor de `alvo` no cursor (`#rrggbb`; vazio = automática / sem cor).
pub fn cor_de(app: &App, alvo: AlvoCor) -> String {
    let f = &app.visao.formato;
    match alvo {
        AlvoCor::Texto => f.cor.clone(),
        AlvoCor::Realce => f.realce.clone(),
        AlvoCor::Fundo => f.fundo.clone(),
        AlvoCor::Pagina => match app.sessao.documento().extras.cor_pagina {
            0 => String::new(),
            c => format!("#{:06x}", c >> 8),
        },
        AlvoCor::Celula => match app.sessao.tabela_celula_do_cursor().map_or(0, |c| c.fundo) {
            0 => String::new(),
            c => format!("#{:06x}", c >> 8),
        },
    }
}

/// O ícone do que se pinta e a amostra da cor atual, que abre o seletor. `largura` 0 = só a
/// amostra (faixa estreita); com largura, o hex ao lado.
pub fn seletor<'a>(app: &App, alvo: AlvoCor, nome_icone: &str, dica: &str, largura: f32) -> Element<'a, Msg> {
    let atual = cor_de(app, alvo);
    // Sem cor, a amostra do texto fica preta e o resto branco (como no papel).
    let amostra = match (atual.is_empty(), alvo) {
        (false, _) => atual.as_str(),
        (true, AlvoCor::Texto) => "#000000",
        (true, _) => "#ffffff",
    };
    let mut conteudo = row![quadradinho(amostra)].spacing(6).align(Align::Center);
    if largura > 0. {
        conteudo = conteudo.push(text(if atual.is_empty() { tr_sem_cor(alvo) } else { atual.clone() }).size(12));
    }
    let campo = button(conteudo).padding([3, 5]).width(if largura > 0. { largura } else { 30. })
        .style(estilo_botao_pequeno(app.menu_faixa == Some(MenuFaixa::Cor(alvo))))
        .on_press(Msg::MenuFaixa(Some(MenuFaixa::Cor(alvo))));
    // A dica só no ícone: no campo ela ficaria por cima do seletor aberto.
    let icone = feather_ui::formatacao::dica(container(icone(nome_icone, 16.)).padding([0, 2]), crate::localidade::tr(dica));
    row![icone, campo].spacing(4).align(Align::Center).into()
}

fn tr_sem_cor(alvo: AlvoCor) -> String {
    crate::localidade::tr(if alvo == AlvoCor::Texto { "Automática" } else { "Sem cor" }).to_string()
}

/// O título do seletor aberto.
pub fn titulo(alvo: AlvoCor) -> &'static str {
    crate::localidade::tr(match alvo {
        AlvoCor::Texto => "Cor do texto",
        AlvoCor::Realce => "Marca-texto",
        AlvoCor::Fundo | AlvoCor::Celula => "Sombreamento",
        AlvoCor::Pagina => "Cor da página",
    })
}

/// As cores prontas dos menus clássicos (o seletor inteiro fica na faixa): nome e `#rrggbb`
/// (vazio = automática / sem cor).
pub fn prontas(alvo: AlvoCor) -> &'static [(&'static str, &'static str)] {
    match alvo {
        AlvoCor::Texto => &[("Automática", ""), ("Preto", "#000000"), ("Cinza", "#5f6368"), ("Vermelho", "#d93025"),
            ("Laranja", "#e8710a"), ("Verde", "#188038"), ("Azul", "#1a73e8"), ("Roxo", "#7b1fa2")],
        AlvoCor::Realce => &[("Sem cor", ""), ("Amarelo", "#ffff00"), ("Verde-limão", "#00ff00"), ("Turquesa", "#00ffff"),
            ("Rosa", "#ff00ff"), ("Laranja", "#ffc000")],
        AlvoCor::Fundo | AlvoCor::Pagina | AlvoCor::Celula => &[("Sem cor", ""), ("Cinza-claro", "#f1f3f4"), ("Amarelo-claro", "#fff8e1"),
            ("Verde-claro", "#e6f4ea"), ("Azul-claro", "#e8f0fe"), ("Rosa-claro", "#fce8e6")],
    }
}

/// A cor de cada item pronto, num quadradinho, para os menus.
pub fn quadradinho(cor: &str) -> Element<'static, Msg> {
    let c = Color::hex(cor);
    container(pelicano::Space::new(14., 14.))
        .style(move |_| pelicano::Style::default()
            .background(c.unwrap_or(Color::TRANSPARENT))
            .border(pelicano::Border::rounded(3.).color(COR_TEXTO_MUTED(), 1.)))
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cores_prontas_validas() {
        for alvo in [AlvoCor::Texto, AlvoCor::Realce, AlvoCor::Fundo, AlvoCor::Pagina, AlvoCor::Celula] {
            assert!(prontas(alvo).iter().all(|(_, c)| c.is_empty() || Color::hex(c).is_some()));
        }
    }
}
