//! As cores da faixa com o seletor nativo do pelicano (`color_picker`: quadro de saturação e
//! brilho, matiz, cores prontas e hex no próprio popup), no jeito do Word: o ícone do que se pinta
//! e, ao lado, a amostra com a cor atual que abre o seletor. A mudança vira `Msg::CorEscolhida`.
use crate::app::{App, Msg};
use feather_ui::{icones::icone, tema::COR_TEXTO_MUTED};
use pelicano::{container, row, Align, Color, Element};

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

/// `#rrggbb` da cor do seletor (o motor lê esse formato; a transparência não vem do seletor).
pub fn hex_de(c: Color) -> String {
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

/// A amostra fechada: sem cor, o texto aparece preto e o resto branco (como no papel).
fn amostra(app: &App, alvo: AlvoCor) -> Color {
    Color::hex(&cor_de(app, alvo)).unwrap_or(if alvo == AlvoCor::Texto { Color::BLACK } else { Color::WHITE })
}

/// O ícone do que se pinta e o seletor nativo com a cor atual. `largura` 0 = só a amostra (faixa
/// estreita): o hex do campo fica cortado.
pub fn seletor<'a>(app: &App, alvo: AlvoCor, nome_icone: &str, dica: &str, largura: f32) -> Element<'a, Msg> {
    let campo = pelicano::color_picker(amostra(app, alvo), move |c| Msg::CorEscolhida(alvo, hex_de(c)));
    let campo: Element<'a, Msg> = if largura > 0. {
        campo.width(largura).into()
    } else {
        container(campo.width(110.)).width(30.).clip(true).into()
    };
    // A dica só no ícone: no campo ela ficaria por cima do seletor aberto.
    let icone = feather_ui::formatacao::dica(container(icone(nome_icone, 16.)).padding([0, 2]), crate::localidade::tr(dica));
    row![icone, campo].spacing(4).align(Align::Center).into()
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
    fn hex_da_cor_do_seletor() {
        assert_eq!(hex_de(Color::hex("#112233").unwrap()), "#112233");
        assert_eq!(hex_de(Color::rgb8(255, 238, 0)), "#ffee00");
        for alvo in [AlvoCor::Texto, AlvoCor::Realce, AlvoCor::Fundo, AlvoCor::Pagina, AlvoCor::Celula] {
            assert!(prontas(alvo).iter().all(|(_, c)| c.is_empty() || Color::hex(c).is_some()));
        }
    }
}
