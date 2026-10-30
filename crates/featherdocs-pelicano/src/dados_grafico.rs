//! A janela "Dados do gráfico": uma planilha pequena como a do FeatherSheet (colunas A, B, C…,
//! linhas 1, 2, 3…), a barra da célula, o intervalo do gráfico (`A1:D5`, digitado ou escolhido
//! arrastando o mouse) e a prévia ao lado. Cada mudança redesenha o gráfico no documento.
use crate::compat::keyboard::{self, key::Named, Modificadores as _};
use crate::faixa::GRAFICOS;
use crate::localidade::tr;
use feather_ui::{icones::icone, tema::*};
use featherdocs_io::grafico_docs::{escrever_intervalo, letra_da_coluna, ler_intervalo, DadosGrafico, Tipo};
use pelicano::{button, column, container, mouse_area, row, scrollable, text, text_input, Align, Border, Color, Direction, Element, Length, Space, Style};

/// O mínimo de colunas e linhas à vista (a grade cresce com os dados).
const COLUNAS: usize = 10;
const LINHAS: usize = 20;
const LARGURA: f32 = 88.;
const ALTURA: f32 = 26.;
pub const ID_BARRA: &str = "dados-grafico-barra";

/// Uma célula: (linha, coluna), a partir de 0.
type Cel = (usize, usize);

/// O gráfico aberto na janela: o bloco da imagem, os dados, a seleção e o erro do motor.
pub struct Edicao {
    pub bloco: u64,
    pub dados: DadosGrafico,
    pub erro: Option<String>,
    /// A seleção: onde começou e onde está.
    pub ancora: Cel,
    pub foco: Cel,
    arrastando: bool,
    /// O texto do campo de intervalo enquanto se digita (pode estar incompleto).
    intervalo_digitado: Option<String>,
}

impl Edicao {
    pub fn nova(bloco: u64, dados: DadosGrafico) -> Self {
        Edicao { bloco, dados, erro: None, ancora: (0, 0), foco: (0, 0), arrastando: false, intervalo_digitado: None }
    }
    fn selecao(&self) -> (Cel, Cel) {
        let (a, f) = (self.ancora, self.foco);
        ((a.0.min(f.0), a.1.min(f.1)), (a.0.max(f.0), a.1.max(f.1)))
    }
    fn celula(&self, (l, c): Cel) -> &str {
        self.dados.grade.get(l).and_then(|r| r.get(c)).map_or("", String::as_str)
    }
    fn definir(&mut self, (l, c): Cel, v: String) {
        let g = &mut self.dados.grade;
        if g.len() <= l {
            g.resize(l + 1, Vec::new());
        }
        if g[l].len() <= c {
            g[l].resize(c + 1, String::new());
        }
        g[l][c] = v;
    }
    fn mover(&mut self, dl: isize, dc: isize, estender: bool) {
        let f = ((self.foco.0 as isize + dl).max(0) as usize, (self.foco.1 as isize + dc).max(0) as usize);
        self.foco = f;
        if !estender {
            self.ancora = f;
        }
    }
}

#[derive(Clone, Debug)]
pub enum Evento {
    Tipo(Tipo),
    Titulo(String),
    Pressionar(usize, usize),
    Entrar(usize, usize),
    Soltar,
    /// Duplo clique: edita a célula na barra.
    Editar(usize, usize),
    /// O texto da barra (a célula ativa).
    Barra(String),
    BarraEnter,
    Intervalo(String),
    UsarSelecao,
    Tecla(keyboard::Key, keyboard::Modifiers),
    Colar(Option<String>),
}

/// O que o app faz depois do evento.
#[derive(Debug, PartialEq)]
pub enum Acao {
    Nada,
    /// Os dados mudaram: redesenhar o gráfico.
    Redesenhar,
    Copiar(String),
    LerClipboard,
    FocarBarra,
}

pub fn aplicar(ed: &mut Edicao, e: Evento) -> Acao {
    match e {
        Evento::Tipo(t) => ed.dados.tipo = t,
        Evento::Titulo(t) => ed.dados.titulo = t,
        Evento::Pressionar(l, c) => {
            (ed.ancora, ed.foco, ed.arrastando) = ((l, c), (l, c), true);
            return Acao::Nada;
        }
        Evento::Entrar(l, c) if ed.arrastando => {
            ed.foco = (l, c);
            return Acao::Nada;
        }
        Evento::Soltar => {
            ed.arrastando = false;
            return Acao::Nada;
        }
        Evento::Editar(l, c) => {
            (ed.ancora, ed.foco, ed.arrastando) = ((l, c), (l, c), false);
            return Acao::FocarBarra;
        }
        Evento::Barra(v) => ed.definir(ed.ancora, v),
        Evento::BarraEnter => {
            ed.mover(1, 0, false);
            ed.ancora = ed.foco;
            return Acao::Nada;
        }
        Evento::Intervalo(t) => {
            // Só redesenha quando o texto já é um intervalo; o resto fica no campo.
            let valido = ler_intervalo(&t).is_some();
            ed.intervalo_digitado = Some(t.clone());
            if !valido {
                return Acao::Nada;
            }
            ed.dados.intervalo = t.trim().to_uppercase();
        }
        Evento::UsarSelecao => {
            let (a, b) = ed.selecao();
            ed.dados.intervalo = escrever_intervalo(a, b);
            ed.intervalo_digitado = None;
        }
        Evento::Colar(Some(t)) => {
            // Texto com tabulações e linhas (o que o FeatherSheet, o Excel e o LibreOffice copiam).
            let (l0, c0) = ed.selecao().0;
            for (i, linha) in t.trim_end_matches(['\n', '\r']).lines().enumerate() {
                for (j, v) in linha.split('\t').enumerate() {
                    ed.definir((l0 + i, c0 + j), v.trim_end_matches('\r').to_string());
                }
            }
        }
        Evento::Colar(None) | Evento::Entrar(..) => return Acao::Nada,
        Evento::Tecla(k, m) => return tecla(ed, k, m),
    }
    Acao::Redesenhar
}

/// Teclas com a grade em foco (nenhum campo de texto pegou a tecla).
fn tecla(ed: &mut Edicao, k: keyboard::Key, m: keyboard::Modifiers) -> Acao {
    use keyboard::Key;
    let ctrl = |c: &str| m.control() && matches!(&k, Key::Character(x) if x.as_str().eq_ignore_ascii_case(c));
    if ctrl("c") || ctrl("x") {
        let (a, b) = ed.selecao();
        let texto = (a.0..=b.0).map(|l| (a.1..=b.1).map(|c| ed.celula((l, c))).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n");
        if ctrl("x") {
            limpar(ed);
        }
        return Acao::Copiar(texto);
    }
    if ctrl("v") {
        return Acao::LerClipboard;
    }
    match &k {
        Key::Named(Named::ArrowUp) => ed.mover(-1, 0, m.shift()),
        Key::Named(Named::ArrowDown | Named::Enter) => ed.mover(1, 0, m.shift() && !matches!(k, Key::Named(Named::Enter))),
        Key::Named(Named::ArrowLeft) => ed.mover(0, -1, m.shift()),
        Key::Named(Named::ArrowRight) => ed.mover(0, 1, m.shift()),
        Key::Named(Named::Tab) => ed.mover(0, if m.shift() { -1 } else { 1 }, false),
        Key::Named(Named::F2) => return Acao::FocarBarra,
        Key::Named(Named::Delete | Named::Backspace) => {
            limpar(ed);
            return Acao::Redesenhar;
        }
        // Digitar sobre a célula troca o conteúdo dela e segue na barra, como na planilha.
        Key::Character(c) if !m.control() && !m.alt() => {
            ed.foco = ed.ancora;
            ed.definir(ed.ancora, c.to_string());
            return Acao::FocarBarra;
        }
        _ => {}
    }
    Acao::Nada
}

fn limpar(ed: &mut Edicao) {
    let (a, b) = ed.selecao();
    for l in a.0..=b.0 {
        for c in a.1..=b.1 {
            if !ed.celula((l, c)).is_empty() {
                ed.definir((l, c), String::new());
            }
        }
    }
}

/// A janela. `previa`: a imagem do gráfico no documento (do cache das páginas).
pub fn view<'a>(ed: Option<&'a Edicao>, previa: Option<&pelicano::Image>) -> Element<'a, Evento> {
    let Some(ed) = ed else {
        return container(text(tr("Dê dois cliques num gráfico do documento para ver os dados dele.")).size(14).color(COR_TEXTO_MUTED()))
            .padding(30).into();
    };
    let rotulo = |t: &str| text(tr(t).to_string()).size(11).color(COR_TEXTO_MUTED());
    let tipos = row(GRAFICOS.iter().map(|&(t, nome, ic)| {
        feather_ui::formatacao::dica(
            button(icone(ic, 18.)).padding([6, 8]).style(estilo_botao_pequeno(t == ed.dados.tipo)).on_press(Evento::Tipo(t)),
            tr(nome),
        )
    })).spacing(4);
    let (sel_a, sel_b) = ed.selecao();
    let intervalo = ler_intervalo(&ed.dados.intervalo);
    let topo = row![
        column![rotulo("Tipo de gráfico"), tipos].spacing(5),
        column![
            rotulo("Título"),
            text_input(tr("Título do gráfico"), ed.dados.titulo.clone()).on_input(Evento::Titulo)
                .size(13).padding([7, 10]).width(Length::FILL).style(estilo_entrada_suspensa),
        ].spacing(5).width(Length::FILL),
        column![
            rotulo("Dados do gráfico"),
            row![
                text_input("A1:D5", ed.intervalo_digitado.clone().unwrap_or_else(|| ed.dados.intervalo.clone()))
                    .on_input(Evento::Intervalo).size(13).padding([7, 10]).width(96).style(estilo_entrada_suspensa),
                button(text(tr("Usar seleção")).size(12)).padding([7, 10]).style(estilo_botao_secundario)
                    .on_press_maybe((sel_a != sel_b).then_some(Evento::UsarSelecao)),
            ].spacing(6),
        ].spacing(5),
    ].spacing(16).align(Align::End);

    let nome = if sel_a == sel_b {
        format!("{}{}", letra_da_coluna(sel_a.1), sel_a.0 + 1)
    } else {
        escrever_intervalo(sel_a, sel_b)
    };
    let barra = row![
        container(text(nome).size(12)).width(84).padding([7, 8]).style(estilo_painel_miniaturas),
        text_input("", ed.celula(ed.ancora).to_string()).id(ID_BARRA).on_input(Evento::Barra).on_submit(Evento::BarraEnter)
            .size(13).padding([7, 10]).width(Length::FILL).style(estilo_entrada_suspensa),
    ].spacing(6);

    // A planilha: cabeçalhos, a seleção (acento) e o intervalo do gráfico (azul claro).
    let colunas = ed.dados.grade.iter().map(Vec::len).max().unwrap_or(0).max(COLUNAS).max(sel_b.1 + 2);
    let linhas = ed.dados.grade.len().max(LINHAS).max(sel_b.0 + 2);
    let cabeca = |t: String, w: f32| {
        Element::from(container(text(t).size(11).color(COR_TEXTO_MUTED())).width(w).height(ALTURA).align_x(Align::Center).align_y(Align::Center)
            .style(|_| Style::default().background(COR_PAINEL()).border(Border::default().color(COR_BORDA_SUAVE(), 0.5))))
    };
    let mut grade = column![row((0..=colunas).map(|c| if c == 0 { cabeca(String::new(), 40.) } else { cabeca(letra_da_coluna(c - 1), LARGURA) }))];
    for l in 0..linhas {
        let mut linha = row![cabeca((l + 1).to_string(), 40.)];
        for c in 0..colunas {
            let selecionada = (sel_a.0..=sel_b.0).contains(&l) && (sel_a.1..=sel_b.1).contains(&c);
            let no_grafico = intervalo.is_some_and(|(a, b)| (a.0..=b.0).contains(&l) && (a.1..=b.1).contains(&c));
            let ativa = (l, c) == ed.ancora;
            let fundo = if selecionada && !ativa { COR_AZUL_FUNDO() } else if no_grafico { Color::rgb8(0xe8, 0xf1, 0xfb) } else { COR_FUNDO_JANELA() };
            let borda = if ativa { Border::default().color(COR_AZUL_ACENTO(), 2.) } else { Border::default().color(COR_BORDA_SUAVE(), 0.5) };
            let celula = container(text(ed.celula((l, c)).to_string()).size(12)).width(LARGURA).height(ALTURA).padding([5, 6]).clip(true)
                .style(move |_| Style::default().background(fundo).border(borda));
            linha = linha.push(mouse_area(celula).on_press(Evento::Pressionar(l, c)).on_enter(Evento::Entrar(l, c))
                .on_release(Evento::Soltar).on_double_click(Evento::Editar(l, c)));
        }
        grade = grade.push(linha);
    }
    let planilha = container(
        scrollable(grade).scrollbar_width(10).scrollbar_track(COR_TRILHO_ROLAGEM()).scrollbar_color(COR_POLEGAR_ROLAGEM())
            .direction(Direction::Both).width(Length::FILL).height(Length::FILL),
    ).width(Length::FILL).height(Length::FILL).style(estilo_painel_miniaturas);

    let imagem: Element<'_, Evento> = match previa {
        Some(i) => pelicano::image(i.clone()).width(Length::FILL).into(),
        None => Space::new(Length::FILL, 120.).into(),
    };
    let aviso = match &ed.erro {
        Some(e) => text(tr(e).to_string()).size(11).color(COR_ERRO()),
        None => text(tr(if ed.dados.tipo == Tipo::Pizza { "A pizza usa a primeira série." } else { "No intervalo, a primeira linha dá os nomes das séries e a primeira coluna as categorias." }))
            .size(11).color(COR_TEXTO_MUTED()),
    };
    let lateral = container(column![rotulo("Prévia"), imagem, aviso].spacing(8)).width(330).padding(10).style(estilo_painel_miniaturas);
    let dica = rotulo("Arraste para escolher células · digite para trocar · Ctrl+V cola do FeatherSheet ou do Excel · Delete apaga");
    container(column![topo, barra, row![planilha, lateral].spacing(12).height(Length::FILL), dica].spacing(12))
        .padding(16).width(Length::FILL).height(Length::FILL)
        .style(|_| Style::default().background(COR_FUNDO_JANELA()))
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;
    use keyboard::{Key, Modifiers};

    fn ed() -> Edicao {
        Edicao::nova(1, DadosGrafico::exemplo(Tipo::Colunas))
    }

    #[test]
    fn arrastar_e_usar_a_selecao_como_intervalo() {
        let mut e = ed();
        aplicar(&mut e, Evento::Pressionar(1, 0));
        aplicar(&mut e, Evento::Entrar(3, 1));
        aplicar(&mut e, Evento::Soltar);
        // Depois de soltar, passar o mouse não muda a seleção.
        aplicar(&mut e, Evento::Entrar(9, 9));
        assert_eq!(aplicar(&mut e, Evento::UsarSelecao), Acao::Redesenhar);
        assert_eq!(e.dados.intervalo, "A2:B4");
        // Intervalo digitado: incompleto não troca; completo troca.
        assert_eq!(aplicar(&mut e, Evento::Intervalo("c".into())), Acao::Nada);
        assert_eq!(e.dados.intervalo, "A2:B4");
        aplicar(&mut e, Evento::Intervalo("a1:c3".into()));
        assert_eq!(e.dados.intervalo, "A1:C3");
    }

    #[test]
    fn teclado_colar_copiar_e_apagar() {
        let mut e = ed();
        aplicar(&mut e, Evento::Pressionar(5, 0));
        aplicar(&mut e, Evento::Soltar);
        aplicar(&mut e, Evento::Colar(Some("Categoria 5\t1\t2\t3\r\nCategoria 6\t4\t5\t6\n".into())));
        assert_eq!(e.dados.grade[6], ["Categoria 6", "4", "5", "6"]);
        aplicar(&mut e, Evento::Tecla(Key::Named(Named::ArrowRight), Modifiers::default()));
        aplicar(&mut e, Evento::Tecla(Key::Named(Named::ArrowDown), Modifiers::SHIFT));
        assert_eq!(e.selecao(), ((5, 1), (6, 1)));
        assert_eq!(aplicar(&mut e, Evento::Tecla(Key::Character("c".into()), Modifiers::CONTROL)), Acao::Copiar("1\n4".into()));
        aplicar(&mut e, Evento::Tecla(Key::Named(Named::Delete), Modifiers::default()));
        assert_eq!((e.dados.grade[5][1].as_str(), e.dados.grade[6][1].as_str()), ("", ""));
        // Digitar troca a célula ativa e passa para a barra.
        assert_eq!(aplicar(&mut e, Evento::Tecla(Key::Character("7".into()), Modifiers::default())), Acao::FocarBarra);
        assert_eq!(e.dados.grade[5][1], "7");
        aplicar(&mut e, Evento::Barra("75".into()));
        aplicar(&mut e, Evento::BarraEnter);
        assert_eq!((e.dados.grade[5][1].as_str(), e.ancora), ("75", (6, 1)));
    }
}
