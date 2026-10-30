//! A janela "Dados do gráfico", como a do FeatherSlide: o tipo, o título e a grade de valores
//! (categorias nas linhas, séries nas colunas). Cada mudança redesenha o gráfico no documento.
use crate::faixa::GRAFICOS;
use crate::localidade::tr;
use feather_ui::{icones::icone, tema::*};
use featherdocs_io::grafico_docs::{DadosGrafico, Tipo};
use pelicano::{button, column, container, row, scrollable, text, text_input, Align, Direction, Element, Length, Space, Style};

/// Até 16 séries, como no motor (`grafico_docs::MAX_SERIES`).
const MAX_SERIES: usize = 16;

/// O gráfico aberto na janela: o bloco da imagem, os dados digitados e o erro, se o motor recusou.
pub struct Edicao {
    pub bloco: u64,
    pub dados: DadosGrafico,
    pub erro: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Evento {
    Tipo(Tipo),
    Titulo(String),
    Celula(usize, usize, String),
    LinhaMais,
    LinhaMenos,
    SerieMais,
    SerieMenos,
}

/// Aplica o evento aos dados; `false` se nada mudou (limite de linhas ou séries).
pub fn aplicar(d: &mut DadosGrafico, e: Evento) -> bool {
    let g = &mut d.grade;
    match e {
        Evento::Tipo(t) => d.tipo = t,
        Evento::Titulo(t) => d.titulo = t,
        Evento::Celula(l, c, v) => match g.get_mut(l).and_then(|l| l.get_mut(c)) {
            Some(cel) => *cel = v,
            None => return false,
        },
        Evento::LinhaMais => {
            let n = g.len();
            let mut nova = vec![String::new(); g[0].len()];
            nova[0] = format!("{} {n}", tr("Categoria"));
            g.push(nova);
        }
        Evento::LinhaMenos if g.len() > 2 => {
            g.pop();
        }
        Evento::SerieMais if g[0].len() <= MAX_SERIES => {
            let n = g[0].len();
            for (i, l) in g.iter_mut().enumerate() {
                l.push(if i == 0 { format!("{} {n}", tr("Série")) } else { "0".into() });
            }
        }
        Evento::SerieMenos if g[0].len() > 2 => {
            for l in g.iter_mut() {
                l.pop();
            }
        }
        _ => return false,
    }
    true
}

pub fn view(ed: Option<&Edicao>) -> Element<'_, Evento> {
    let Some(ed) = ed else {
        return container(text(tr("Selecione um gráfico e clique em Inserir → Gráfico → Editar dados.")).size(14).color(COR_TEXTO_MUTED()))
            .padding(30).into();
    };
    let g = &ed.dados.grade;
    let tipos = row(GRAFICOS.iter().map(|&(t, nome, ic)| {
        feather_ui::formatacao::dica(
            button(icone(ic, 18.)).padding([6, 8]).style(estilo_botao_pequeno(t == ed.dados.tipo)).on_press(Evento::Tipo(t)),
            tr(nome),
        )
    })).spacing(4);
    let topo = column![
        row![
            icone("chart-bar", 22.),
            text(tr("Dados do gráfico")).size(19),
            Space::new(Length::FILL, Length::Shrink),
            text(format!("{} {} · {} {}", g.len() - 1, tr("categorias"), g[0].len() - 1, tr("séries"))).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(10).align(Align::Center),
        row![
            column![text(tr("Tipo de gráfico")).size(11).color(COR_TEXTO_MUTED()), tipos].spacing(5),
            column![
                text(tr("Título")).size(11).color(COR_TEXTO_MUTED()),
                text_input(tr("Título do gráfico"), ed.dados.titulo.clone()).on_input(Evento::Titulo)
                    .size(13).padding([7, 10]).width(Length::FILL).style(estilo_entrada_suspensa),
            ].spacing(5).width(Length::FILL),
        ].spacing(16).align(Align::End),
    ].spacing(16);

    let mut grade = column![].spacing(0);
    for (l, linha) in g.iter().enumerate() {
        let mut celulas = row![container(text(if l == 0 { "#".into() } else { l.to_string() }).size(11).color(COR_TEXTO_MUTED()))
            .width(34).padding([9, 8])].spacing(4);
        for (c, valor) in linha.iter().enumerate() {
            celulas = celulas.push(if l == 0 && c == 0 {
                Element::from(container(text(tr("Categoria")).size(12)).width(170).padding([9, 8]))
            } else {
                let dica = if l == 0 { "Série" } else if c == 0 { "Categoria" } else { "0" };
                text_input(tr(dica), valor.clone()).on_input(move |s| Evento::Celula(l, c, s))
                    .padding([7, 9]).size(12).width(if c == 0 { 170 } else { 110 }).style(estilo_entrada_suspensa).into()
            });
        }
        let fundo = if l == 0 { COR_AZUL_FUNDO() } else if l % 2 == 0 { COR_FUNDO_JANELA() } else { COR_PAINEL() };
        grade = grade.push(container(celulas.align(Align::Center)).padding([5, 8]).style(move |_| Style::default().background(fundo)));
    }
    let tabela = container(
        scrollable(grade).scrollbar_width(10).scrollbar_track(COR_TRILHO_ROLAGEM()).scrollbar_color(COR_POLEGAR_ROLAGEM())
            .direction(Direction::Both).height(Length::FILL),
    ).padding(8).width(Length::FILL).height(Length::FILL).style(estilo_painel_miniaturas);

    let botao = |ic: &str, rotulo: &str, e: Evento, sim: bool| {
        button(row![icone(ic, 14.), text(tr(rotulo).to_string()).size(12)].spacing(6).align(Align::Center))
            .padding([7, 10]).style(estilo_botao_secundario).on_press_maybe(sim.then_some(e))
    };
    let acoes = row![
        botao("plus", "Categoria", Evento::LinhaMais, true),
        botao("minus", "Categoria", Evento::LinhaMenos, g.len() > 2),
        Space::new(10, Length::Shrink),
        botao("plus", "Série", Evento::SerieMais, g[0].len() <= MAX_SERIES),
        botao("minus", "Série", Evento::SerieMenos, g[0].len() > 2),
    ].spacing(6);
    let rodape = match &ed.erro {
        Some(e) => text(tr(e).to_string()).size(11).color(COR_ERRO()),
        None => text(tr(if ed.dados.tipo == Tipo::Pizza {
            "A pizza usa a primeira série. As mudanças aparecem no documento na hora."
        } else {
            "Use vírgula ou ponto nos decimais. As mudanças aparecem no documento na hora."
        })).size(11).color(COR_TEXTO_MUTED()),
    };
    container(column![topo, tabela, acoes, rodape].spacing(14))
        .padding(16).width(Length::FILL).height(Length::FILL)
        .style(|_| Style::default().background(COR_FUNDO_JANELA()))
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn linhas_e_series_respeitam_os_limites() {
        let mut d = DadosGrafico::exemplo(Tipo::Colunas);
        assert!(aplicar(&mut d, Evento::LinhaMais));
        assert_eq!((d.grade.len(), d.grade[5][0].as_str()), (6, "Categoria 5"));
        assert!(aplicar(&mut d, Evento::SerieMais));
        assert_eq!(d.grade[0][4], "Série 4");
        while aplicar(&mut d, Evento::LinhaMenos) {}
        while aplicar(&mut d, Evento::SerieMenos) {}
        assert_eq!((d.grade.len(), d.grade[0].len()), (2, 2));
        while aplicar(&mut d, Evento::SerieMais) {}
        assert_eq!(d.grade[0].len(), MAX_SERIES + 1);
        assert!(!aplicar(&mut d, Evento::Celula(99, 0, "x".into())));
    }
}
