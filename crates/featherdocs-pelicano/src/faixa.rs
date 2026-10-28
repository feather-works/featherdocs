//! Faixa com componentes e ícones da suíte; cada comando é executado pelo motor.
use crate::{
    app::{App, Comando, Msg},
    cor_ui::AlvoCor,
    localidade::{lista, tr},
};
use feather_ui::{
    componentes::{botao_grande, botao_icone, botao_pequeno, grupo_faixa, lista_suspensa},
    icones::icone,
    tema::*,
};
use crate::dialogos::Tipo as Form;
use featherdocs_io::dividido::Fonte;
use crate::compat::button;
use pelicano::{column, container, horizontal_space, row, scrollable, text, Align, Border, Element, Font, Length, Padding, Space, Status, Style, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aba {
    Inicio,
    Inserir,
    Desenhar,
    Design,
    Layout,
    Referencias,
    Correspondencias,
    Revisao,
    Exibir,
}
impl Aba {
    /// A ordem das abas do Word.
    pub const TODAS: [Self; 9] = [
        Self::Inicio,
        Self::Inserir,
        Self::Desenhar,
        Self::Design,
        Self::Layout,
        Self::Referencias,
        Self::Correspondencias,
        Self::Revisao,
        Self::Exibir,
    ];
    pub fn nome(self) -> &'static str {
        tr(match self {
            Self::Inicio => "inicio",
            Self::Inserir => "inserir",
            Self::Desenhar => "desenhar",
            Self::Design => "design",
            Self::Layout => "layout",
            Self::Referencias => "referencias",
            Self::Correspondencias => "correspondencias",
            Self::Revisao => "revisao",
            Self::Exibir => "exibir",
        })
    }
}
fn grande(nome: &str, chave: &str, cmd: Comando, ativo: bool) -> Element<'static, Msg> {
    botao_grande(icone(nome, 24.), tr(chave), Some(Msg::Comando(cmd)), ativo)
}
fn pequeno(nome: &str, chave: &str, cmd: Comando, ativo: bool) -> Element<'static, Msg> {
    botao_pequeno(icone(nome, 16.), tr(chave), Some(Msg::Comando(cmd)), ativo)
}
fn so_icone(nome: &str, dica: &str, cmd: Comando, ativo: bool) -> Element<'static, Msg> {
    botao_icone(icone(nome, 16.), tr(dica), Some(Msg::Comando(cmd)), ativo)
}
/// Menu da faixa: o rótulo escolhido vira o comando na mesma posição de `comandos`.
fn menu(rotulo: &'static str, opcoes: &'static [&'static str], comandos: fn(usize) -> Comando, largura: f32) -> Element<'static, Msg> {
    let opcoes = lista(opcoes);
    lista_suspensa(tr(rotulo), opcoes, None, move |o| {
        Msg::Comando(comandos(opcoes.iter().position(|x| *x == o).unwrap_or(0)))
    }, largura)
}

/// Um comando contextual não deve ser acionado fora de uma célula de tabela.
fn tabela_botao(rotulo: &'static str, comando: Comando, disponivel: bool) -> Element<'static, Msg> {
    let botao = button(text(tr(rotulo)).size(12)).padding([4, 8])
        .style(estilo_botao_pequeno(false));
    let botao = if disponivel { botao.on_press(Msg::Comando(comando)) } else { botao };
    botao.into()
}
/// Controle ainda sem motor: aparece desabilitado, como na especificação da faixa.
fn em_breve(nome: &str, rotulo: &'static str) -> Element<'static, Msg> {
    // shortcut: a dica do pelicano não escolhe o tamanho da letra (era 11 no Iced); pedir `size` no `tooltip`.
    pelicano::tooltip(
        botao_grande(icone(nome, 24.), tr(rotulo), None, false),
        tr("Em breve"),
        pelicano::TooltipPosition::Bottom,
    )
    .padding([3u16, 6])
    .style(estilo_dica)
    .into()
}
fn estilo_dica(_: &Theme) -> Style {
    Style::default()
        .background(COR_PAINEL())
        .border(Border::rounded(4.).color(COR_BORDA(), 1.))
        .text_color(COR_TEXTO())
}

/// Os estilos de fábrica da galeria: id, nome e aparência (corpo, negrito, itálico, mono).
pub(crate) const DE_FABRICA: [(&str, &str, f32, bool, bool, bool); 9] = [
    ("paragrafo", "Normal", 13., false, false, false),
    ("titulo1", "Título 1", 18., true, false, false),
    ("titulo2", "Título 2", 15., true, false, false),
    ("titulo3", "Título 3", 13.5, true, false, false),
    ("titulo4", "Título 4", 13., true, false, false),
    ("titulo5", "Título 5", 12., true, false, false),
    ("titulo6", "Título 6", 11., true, false, false),
    ("citacao", "Citação", 13., false, true, false),
    ("codigo", "Código", 12., false, false, true),
];
/// Quantos blocos cabem na galeria fechada.
pub const POR_LINHA: usize = 4;
/// Quantos blocos por linha na galeria aberta.
const POR_LINHA_ABERTA: usize = 4;
/// O tamanho de cada bloco da galeria.
const BLOCO: (f32, f32) = (92., 54.);
/// A largura da galeria aberta (os blocos, os vãos e a margem do menu).
pub const LARGURA_GALERIA_ABERTA: f32 = (BLOCO.0 + 4.) * POR_LINHA_ABERTA as f32 + 24.;

/// Os menus que abrem por baixo da faixa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuFaixa {
    /// Layout → Quebras.
    Quebras,
    /// A galeria de estilos inteira, com os comandos de estilo.
    Estilos,
    /// Tipos de marcador da lista.
    Marcadores,
    /// Tipos de numeração da lista.
    Numeracao,
    /// Entrelinha e espaço antes e depois do parágrafo.
    Entrelinha,
    /// Bordas do parágrafo.
    Bordas,
    /// Layout: margens, orientação, tamanho do papel, números de linha e alinhamento vertical.
    Margens,
    Orientacao,
    Tamanho,
    NumerosLinha,
    Vertical,
    /// Inserir → Gráfico ▾: os tipos de gráfico a partir da tabela.
    Graficos,
    /// Um grupo recolhido da faixa estreita (índice na aba), aberto inteiro num menu.
    Grupo(u8),
    /// Faixa recolhida: um menu clássico (ARQUIVO, EDITAR…) e o submenu aberto dentro dele.
    Classico(u8, Option<u8>),
}

/// A letra da amostra: a da página (Liberation), não a da interface.
fn fonte_da_amostra(negrito: bool, italico: bool, mono: bool) -> Font {
    Font {
        family: Some(if mono { "Liberation Mono" } else { "Liberation Sans" }),
        weight: if negrito { 700 } else { 400 },
        italic: italico,
    }
}
/// O nome cortado com "…" para caber numa linha do bloco.
fn curto(nome: &str, max: usize) -> String {
    if nome.chars().count() <= max {
        return nome.to_string();
    }
    format!("{}…", nome.chars().take(max - 1).collect::<String>().trim_end())
}
/// Um bloco da galeria no jeito do Word: "AaBbCc" com a aparência do estilo e o nome embaixo,
/// pequeno e numa linha só; todos do mesmo tamanho.
fn bloco_estilo(nome: String, tamanho: f32, fonte: Font, cor: u32, ativo: bool, msg: Msg) -> Element<'static, Msg> {
    let amostra = text("AaBbCc")
        .size(tamanho.clamp(10., 17.))
        .font(fonte)
        .no_wrap();
    let amostra = if cor == 0 { amostra } else { amostra.color(crate::pagina::cor(cor)) };
    let rotulo = text(curto(&nome, 12)).size(10.5).color(COR_TEXTO_MUTED()).no_wrap();
    let dica = nome.clone();
    let bloco = button(
        column![
            container(amostra).width(Length::FILL).align_x(Align::Center).height(Length::FILL).align_y(Align::Center).clip(true),
            container(rotulo).width(Length::FILL).align_x(Align::Center).clip(true),
        ]
        .spacing(1),
    )
    .width(BLOCO.0)
    .height(BLOCO.1)
    .padding([4, 4])
    .on_press(msg)
    .style(move |_, status| Style::default()
        .background(if status == Status::Hovered { COR_HOVER() } else { COR_PAINEL() })
        .text_color(COR_TEXTO())
        .border(Border::rounded(4.).color(if ativo { COR_AZUL_ACENTO() } else { COR_BORDA() }, if ativo { 2. } else { 1. })));
    pelicano::tooltip(bloco, dica, pelicano::TooltipPosition::Bottom)
        .style(estilo_popup_menu)
        .into()
}
/// Todos os blocos da galeria, na ordem: de fábrica, rápidos (os do Word que viram estilo com nome
/// ao primeiro uso) e os estilos com nome do documento que não são um rápido.
fn blocos_de_estilo(app: &App) -> Vec<Element<'static, Msg>> {
    use featherdocs_core::modelo::{CODIGO, ITALICO, NEGRITO};
    let atual = app.visao.formato.estilo.as_str();
    let nomeado = app.sessao.estilo_com_nome_no_cursor();
    let estilos = &app.sessao.documento().estilos;
    let nome_no_cursor = estilos.pegar(nomeado).map(|e| e.nome.to_lowercase());
    let mut blocos: Vec<Element<'static, Msg>> = DE_FABRICA
        .iter()
        .map(|&(id, nome, tamanho, n, i, m)| {
            bloco_estilo(tr(nome).into(), tamanho, fonte_da_amostra(n, i, m), 0, nomeado == 0 && atual == id,
                Msg::Comando(Comando::Estilo(id)))
        })
        .collect();
    for (k, r) in crate::dialogos::RAPIDOS.iter().enumerate() {
        let nome = tr(r.nome);
        let ativo = nome_no_cursor.as_deref() == Some(nome.to_lowercase().as_str());
        blocos.push(bloco_estilo(nome.into(), r.amostra, fonte_da_amostra(false, r.italico, false), r.cor, ativo,
            Msg::Comando(Comando::EstiloRapido(k))));
    }
    for e in &estilos.lista {
        if e.caractere || crate::dialogos::rapido_de_nome(&e.nome).is_some() {
            continue;
        }
        let tamanho = if e.tamanho > 0. { e.tamanho.clamp(9., 18.) } else { 13. };
        let fonte = fonte_da_amostra(e.base > 0 || e.bits & NEGRITO != 0, e.bits & ITALICO != 0, e.bits & CODIGO != 0);
        blocos.push(bloco_estilo(e.nome.clone(), tamanho, fonte, e.cor, e.id == nomeado, Msg::Comando(Comando::EstiloNome(e.id))));
    }
    blocos
}
/// A galeria de estilos no jeito do Word: `n` por vez (4; 2 na faixa estreita), setas para rolar e a
/// de abrir a galeria inteira.
fn galeria_estilos(app: &App, n: usize) -> Element<'_, Msg> {
    let blocos = blocos_de_estilo(app);
    let inicio = app.galeria_estilos.min(blocos.len().saturating_sub(n));
    let fim = inicio + n;
    let total = blocos.len();
    let visiveis: Vec<_> = blocos.into_iter().skip(inicio).take(n).collect();
    let seta = |nome: &str, dica: &str, msg: Option<Msg>| botao_icone(icone(nome, 12.), tr(dica), msg, false);
    let aberta = app.menu_faixa == Some(MenuFaixa::Estilos);
    let comandos = [
        ("plus", "novo_estilo", Comando::Formulario(Form::NovoEstilo)),
        ("refresh", "atualizar_estilo", Comando::EstiloAtualizar),
        ("trash", "excluir_estilo", Comando::EstiloExcluir),
    ];
    // Estreita: só os ícones (o nome fica na dica).
    let comandos: Vec<Element<'_, Msg>> = comandos.into_iter()
        .map(|(ic, chave, c)| if n >= POR_LINHA { pequeno(ic, chave, c, false) } else { so_icone(ic, chave, c, false) })
        .collect();
    column![
        row![
            row(visiveis).spacing(4),
            column![
                seta("chevron-up", "Estilos anteriores", (inicio > 0).then_some(Msg::GaleriaEstilos(-(n as i32)))),
                seta("chevron-down", "Mais estilos", (fim < total).then_some(Msg::GaleriaEstilos(n as i32))),
                botao_icone(icone("layout-grid", 12.), tr("Todos os estilos"),
                    Some(Msg::MenuFaixa((!aberta).then_some(MenuFaixa::Estilos))), aberta),
            ]
            .spacing(1),
        ]
        .spacing(4)
        .align(Align::Center),
        row(comandos).spacing(4),
    ]
    .spacing(4)
    .into()
}

/// O título de uma parte do menu, numa faixa cinza (como "Quebras de Página" no Word).
fn cabecalho_menu(titulo: &str) -> Element<'static, Msg> {
    container(text(tr(titulo).to_string()).size(11).font(Font::SEMIBOLD))
        .width(Length::FILL)
        .padding([4u16, 10])
        .style(|_| Style::default().background(COR_HOVER()).text_color(COR_TEXTO()))
        .into()
}
/// Um item de menu com ícone grande, nome e a explicação embaixo.
fn item_descrito(nome_icone: &str, titulo: &str, descricao: &str, cmd: Comando) -> Element<'static, Msg> {
    button(
        row![
            icone(nome_icone, 26.),
            column![
                text(tr(titulo).to_string()).size(13).font(Font::SEMIBOLD),
                text(tr(descricao).to_string()).size(11).color(COR_TEXTO_MUTED()),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align(Align::Center),
    )
    .width(Length::FILL)
    .padding([6, 10])
    .style(estilo_botao_pequeno(false))
    .on_press(Msg::Comando(cmd))
    .into()
}
/// Um item de menu de uma linha só.
fn item_simples(nome_icone: &str, titulo: &str, cmd: Comando) -> Element<'static, Msg> {
    button(row![icone(nome_icone, 16.), text(tr(titulo).to_string()).size(12)].spacing(10).align(Align::Center))
        .width(Length::FILL)
        .padding([5, 10])
        .style(estilo_botao_pequeno(false))
        .on_press(Msg::Comando(cmd))
        .into()
}

/// Botão que abre um menu da faixa: o ícone e a setinha; clicar de novo fecha.
fn botao_de_menu(app: &App, nome_icone: &str, dica: &str, menu: MenuFaixa) -> Element<'static, Msg> {
    let aberto = app.menu_faixa == Some(menu);
    feather_ui::formatacao::dica(
        button(row![icone(nome_icone, 16.), icone("chevron-down", 10.)].spacing(1).align(Align::Center))
            .padding([4, 4])
            .style(estilo_botao_pequeno(aberto))
            .on_press(Msg::MenuFaixa((!aberto).then_some(menu))),
        tr(dica),
    )
}

/// O grupo Parágrafo, no arranjo do Word: listas e recuo em cima; alinhamento, espaçamento,
/// sombreamento e bordas embaixo.
fn paragrafo(app: &App) -> Element<'_, Msg> {
    let formato = &app.visao.formato;
    let separador = || container(Space::new(1u16, 18u16)).style(|_| Style::default().background(COR_BORDA()));
    column![
        row![
            so_icone("list", "Lista com marcadores (Ctrl+Shift+8)", Comando::Estilo("item"), formato.estilo == "item"),
            botao_de_menu(app, "list", "Tipo de marcador", MenuFaixa::Marcadores),
            so_icone("list-numbers", "Lista numerada (Ctrl+Shift+7)", Comando::Estilo("item_ordenado"), formato.estilo == "item_ordenado"),
            botao_de_menu(app, "list-numbers", "Tipo de numeração", MenuFaixa::Numeracao),
            separador(),
            so_icone("indent-decrease", "Diminuir recuo (Shift+Tab)", Comando::Recuar(-1), false),
            so_icone("indent-increase", "Aumentar recuo (Tab)", Comando::Recuar(1), false),
        ]
        .spacing(2)
        .align(Align::Center),
        row![
            feather_ui::alinhamento::seletor_horizontal(
                feather_ui::alinhamento::Horizontal::de_identificador(&formato.alinhamento),
                true,
                true,
                |escolha| Msg::Comando(Comando::Alinhar(match escolha {
                    feather_ui::alinhamento::Horizontal::Esquerda => "esquerda",
                    feather_ui::alinhamento::Horizontal::Centro => "centro",
                    feather_ui::alinhamento::Horizontal::Direita => "direita",
                    feather_ui::alinhamento::Horizontal::Justificado => "justificado",
                })),
            ),
            separador(),
            botao_de_menu(app, "line-height", "Espaçamento de linha e parágrafo", MenuFaixa::Entrelinha),
            feather_ui::formatacao::dica(
                button(icone("bucket", 16.)).padding([4, 6]).style(estilo_botao_pequeno(false))
                    .on_press(Msg::AbrirCor(AlvoCor::Fundo)),
                tr("Sombreamento"),
            ),
            botao_de_menu(app, "square", "Bordas", MenuFaixa::Bordas),
        ]
        .spacing(2)
        .align(Align::Center),
    ]
    .spacing(8)
    .into()
}

/// Um item de menu com a marca de escolhido à esquerda.
fn item_marcado(titulo: &str, marcado: bool, cmd: Comando) -> Element<'static, Msg> {
    let marca: Element<'static, Msg> = if marcado { icone("check", 14.).into() } else { Space::new(14u16, 14u16).into() };
    button(row![marca, text(tr(titulo).to_string()).size(12)].spacing(10).align(Align::Center))
        .width(Length::FILL)
        .padding([5, 10])
        .style(estilo_botao_pequeno(false))
        .on_press(Msg::Comando(cmd))
        .into()
}

// As listas que a faixa e os menus clássicos (`menu_classico`) mostram: um lugar só para os dois.
pub(crate) const MARCADORES: [(&str, &str); 5] = [("Disco", "padrao"), ("Círculo", "circulo"), ("Quadrado", "quadrado"), ("Seta", "seta"), ("Caixa de tarefa", "tarefa")];
pub(crate) const NUMERACOES: [(&str, &str); 3] = [("1. 2. 3.", "numerica"), ("a. b. c.", "alfabetica"), ("i. ii. iii.", "romana")];
pub(crate) const ENTRELINHAS: [(&str, f32); 7] = [("1,0", 1.0), ("1,15", 1.15), ("1,5", 1.5), ("2,0", 2.0), ("2,5", 2.5), ("3,0", 3.0), ("Padrão", 0.)];
pub(crate) const BORDAS: [(&str, u8); 6] = [("Sem borda", 0), ("Borda inferior", 8), ("Borda superior", 4), ("Borda esquerda", 1), ("Borda direita", 2), ("Contorno", 15)];
pub(crate) const MAIUSCULAS: [(&str, &str); 5] = [("Frase", "frase"), ("minúsculas", "minusculas"), ("MAIÚSCULAS", "maiusculas"), ("Cada Palavra", "palavras"), ("aLTERNAR", "alternar")];
pub(crate) const TABELAS: [(&str, usize, usize); 7] = [("2 × 2", 2, 2), ("4 × 4", 4, 4), ("5 × 5", 5, 5), ("6 × 6", 6, 6), ("4 × 2", 4, 2), ("8 × 4", 8, 4), ("10 × 5", 10, 5)];
pub(crate) const NUMERO_PAGINA: [&str; 8] = ["Embaixo, centro", "Embaixo, direita", "Embaixo, esquerda", "Em cima, centro", "Em cima, direita", "Em cima, esquerda", "Página X de Y", "Remover número"];
pub(crate) fn numero_pagina(i: usize) -> Comando {
    match i {
        6 => Comando::NumeroPagina(tr("Página {pagina} de {total}"), "base_centro"),
        7 => Comando::NumeroRemover,
        _ => Comando::NumeroPagina("{pagina}", ["base_centro", "base_direita", "base_esquerda", "topo_centro", "topo_direita", "topo_esquerda"][i.min(5)]),
    }
}
use featherdocs_io::grafico_docs::Tipo as TipoGrafico;
pub(crate) const GRAFICOS: [(TipoGrafico, &str, &str); 7] = [
    (TipoGrafico::Colunas, "Colunas", "chart-columns"),
    (TipoGrafico::Barras, "Barras", "chart-bar"),
    (TipoGrafico::Linhas, "Linhas", "chart-line"),
    (TipoGrafico::Area, "Área", "chart-area"),
    (TipoGrafico::Pizza, "Pizza", "chart-pie"),
    (TipoGrafico::Dispersao, "Dispersão", "chart-scatter"),
    (TipoGrafico::Degraus, "Degraus", "arrow-step"),
];
/// Margens prontas do Word: nome, o que mostram e os valores em cm (esquerda, direita, superior, inferior).
pub(crate) const MARGENS: [(&str, &str, [f32; 4]); 5] = [
    ("Normal", "Superior: 2,54 cm · Inferior: 2,54 cm · Esquerda: 2,54 cm · Direita: 2,54 cm", [2.54; 4]),
    ("Estreita", "Superior: 1,27 cm · Inferior: 1,27 cm · Esquerda: 1,27 cm · Direita: 1,27 cm", [1.27; 4]),
    ("Moderada", "Superior: 2,54 cm · Inferior: 2,54 cm · Esquerda: 1,91 cm · Direita: 1,91 cm", [1.91, 1.91, 2.54, 2.54]),
    ("Larga", "Superior: 2,54 cm · Inferior: 2,54 cm · Esquerda: 5,08 cm · Direita: 5,08 cm", [5.08, 5.08, 2.54, 2.54]),
    ("ABNT", "Superior: 3 cm · Inferior: 2 cm · Esquerda: 3 cm · Direita: 2 cm", [3.0, 2.0, 3.0, 2.0]),
];
/// A página tem estas margens (cm, na ordem de `MARGENS`)?
pub(crate) fn margens_iguais(p: &featherdocs_core::modelo::ConfigPagina, cm: [f32; 4]) -> bool {
    let pt = cm.map(|v| v * 72. / 2.54);
    [p.margem_esq, p.margem_dir, p.margem_sup, p.margem_inf].iter().zip(pt).all(|(a, b)| (a - b).abs() < 0.6)
}
/// Os papéis que o motor conhece (`tamanho_do_papel`): id, nome e as medidas em retrato.
pub(crate) const PAPEIS: [(&str, &str, &str); 5] = [
    ("a4", "A4", "21 cm × 29,7 cm"),
    ("carta", "Carta", "21,59 cm × 27,94 cm"),
    ("oficio", "Ofício", "21,59 cm × 35,56 cm"),
    ("a3", "A3", "29,7 cm × 42 cm"),
    ("a5", "A5", "14,8 cm × 21 cm"),
];
use featherdocs_core::modelo::{NumeracaoDeLinhas, Vertical as Vert};
pub(crate) const NUMEROS_DE_LINHA: [(&str, &str, NumeracaoDeLinhas); 3] = [
    ("Nenhum", "nenhuma", NumeracaoDeLinhas::Nenhuma),
    ("Contínuos", "continua", NumeracaoDeLinhas::Continua),
    ("Reiniciar a cada página", "por_pagina", NumeracaoDeLinhas::PorPagina),
];
pub(crate) const VERTICAIS: [(&str, &str, &str, Vert); 3] = [
    ("No topo", "topo", "layout-align-top", Vert::Topo),
    ("No centro", "centro", "layout-align-middle", Vert::Centro),
    ("Na base", "base", "layout-align-bottom", Vert::Base),
];
/// Layout → Quebras: ícone, nome, explicação e o que insere (`pagina`, `linha` ou o tipo do bloco).
pub(crate) const QUEBRAS: [(&str, &str, &str, &str); 7] = [
    ("page-break", "Página", "Marcar o ponto em que uma página termina e a próxima começa.", "pagina"),
    ("columns", "Coluna", "Indicar que o texto após a quebra de coluna começará na coluna seguinte.", "quebra_de_coluna"),
    ("text-wrap", "Disposição do texto", "Separar o texto ao redor dos objetos, como o texto das legendas do corpo do texto.", "linha"),
    ("file-plus", "Próxima página", "Inserir uma quebra de seção e iniciar a nova seção na página seguinte.", "secao_proxima"),
    ("arrows-vertical", "Contínuo", "Inserir uma quebra de seção e iniciar a nova seção na mesma página.", "secao_continua"),
    ("copy", "Página par", "Inserir uma quebra de seção e iniciar a nova seção na próxima página par.", "secao_par"),
    ("copy-plus", "Página ímpar", "Inserir uma quebra de seção e iniciar a nova seção na próxima página ímpar.", "secao_impar"),
];
pub(crate) fn comando_quebra(id: &'static str) -> Comando {
    match id {
        "pagina" => Comando::Quebra,
        "linha" => Comando::QuebraDeLinha,
        bloco => Comando::Estilo(bloco),
    }
}

/// Os menus pequenos do grupo Parágrafo (marcadores, numeração, espaçamento, bordas).
pub fn menu_paragrafo(app: &App, menu: MenuFaixa) -> Element<'static, Msg> {
    let f = &app.visao.formato;
    let mut col = column![].spacing(1).width(240);
    match menu {
        MenuFaixa::Marcadores => {
            col = col.push(cabecalho_menu("Biblioteca de marcadores"));
            for (nome, id) in MARCADORES {
                col = col.push(item_marcado(nome, f.estilo == "item" && f.lista == id, Comando::Lista(id)));
            }
        }
        MenuFaixa::Numeracao => {
            col = col.push(cabecalho_menu("Biblioteca de numeração"));
            for (nome, id) in NUMERACOES {
                col = col.push(item_marcado(nome, f.estilo == "item_ordenado" && f.lista == id, Comando::Lista(id)));
            }
            col = col.push(pelicano::horizontal_rule(1.)).push(item_marcado("Reiniciar numeração em 1", false, Comando::ListaReiniciar));
        }
        MenuFaixa::Entrelinha => {
            col = col.push(cabecalho_menu("Entrelinha"));
            for (nome, v) in ENTRELINHAS {
                col = col.push(item_marcado(nome, (f.entrelinha - v).abs() < 0.01, Comando::Entrelinha(v)));
            }
            col = col.push(cabecalho_menu("Espaço antes"));
            for (nome, v) in [("0 pt", 0.), ("6 pt", 6.), ("12 pt", 12.)] {
                col = col.push(item_marcado(nome, (f.antes - v).abs() < 0.01, Comando::Espaco(true, v)));
            }
            col = col.push(cabecalho_menu("Espaço depois"));
            for (nome, v) in [("0 pt", 0.), ("6 pt", 6.), ("12 pt", 12.)] {
                col = col.push(item_marcado(nome, (f.depois - v).abs() < 0.01, Comando::Espaco(false, v)));
            }
        }
        MenuFaixa::Bordas => {
            col = col.push(cabecalho_menu("Bordas"));
            for (nome, bits) in BORDAS {
                col = col.push(item_marcado(nome, f.borda == bits, Comando::Borda(bits)));
            }
        }
        _ => {}
    }
    col.into()
}

/// Layout → Quebras: as de página e as de seção, como no Word.
pub fn menu_quebras() -> Element<'static, Msg> {
    let mut col = column![cabecalho_menu("Quebras de página")].spacing(1).width(400);
    for (i, (ic, nome, descricao, id)) in QUEBRAS.into_iter().enumerate() {
        if i == 3 {
            col = col.push(cabecalho_menu("Quebras de seção"));
        }
        col = col.push(item_descrito(ic, nome, descricao, comando_quebra(id)));
    }
    col.into()
}

/// A galeria inteira, com salvar como estilo rápido, limpar formatação e aplicar estilos.
pub fn menu_estilos(app: &App) -> Element<'static, Msg> {
    let mut grade = column![].spacing(4);
    let mut blocos = blocos_de_estilo(app).into_iter().peekable();
    while blocos.peek().is_some() {
        let mut linha: Vec<_> = blocos.by_ref().take(POR_LINHA_ABERTA).collect();
        // A última linha completa com vazios para os blocos não mudarem de lugar.
        while linha.len() < POR_LINHA_ABERTA {
            linha.push(Space::new(Length::Fixed(BLOCO.0), Length::Fixed(BLOCO.1)).into());
        }
        grade = grade.push(row(linha).spacing(4));
    }
    column![
        container(scrollable(grade).height(Length::Shrink)).padding(8).max_height(320),
        pelicano::horizontal_rule(1.),
        item_simples("plus", "Salvar seleção como novo estilo rápido…", Comando::Formulario(Form::NovoEstilo)),
        item_simples("clear", "Limpar formatação", Comando::LimparFormatacao),
        item_simples("wand", "Aplicar estilos…", Comando::Formulario(Form::AplicarEstilo)),
    ]
    .spacing(2)
    .width(LARGURA_GALERIA_ABERTA)
    .into()
}

/// Uma folha desenhada (retrato ou paisagem) para os menus de orientação e tamanho.
fn folha(largura: f32, altura: f32) -> Element<'static, Msg> {
    container(
        container(Space::new(largura, altura))
            .style(|_| Style::default().background(COR_PAINEL()).border(Border::rounded(1.).color(COR_TEXTO_MUTED(), 1.2))),
    )
    .width(30)
    .height(30)
    .center()
    .into()
}
/// Item com marca de escolhido, desenho ou ícone à esquerda, nome e a explicação embaixo.
fn item_opcao(figura: Element<'static, Msg>, titulo: &str, descricao: &str, marcado: bool, msg: Option<Msg>) -> Element<'static, Msg> {
    let mut texto = column![text(tr(titulo).to_string()).size(13).font(Font::SEMIBOLD)].spacing(2);
    if !descricao.is_empty() {
        texto = texto.push(text(tr(descricao).to_string()).size(11).color(COR_TEXTO_MUTED()));
    }
    let marca: Element<'static, Msg> = if marcado { icone("check", 14.).into() } else { Space::new(14u16, 14u16).into() };
    button(row![marca, figura, texto].spacing(10).align(Align::Center))
        .width(Length::FILL)
        .padding([5, 10])
        .style(estilo_botao_pequeno(marcado))
        .on_press_maybe(msg)
        .into()
}
pub(crate) fn em_grafico(app: &App) -> bool {
    let bloco = app.sessao.cursor().foco.bloco;
    app.sessao.documento().blocos.iter().any(|b| {
        b.id == bloco && matches!(&b.conteudo, featherdocs_core::modelo::Conteudo::Imagem { origem, .. }
            if featherdocs_io::grafico_docs::eh_grafico(origem))
    })
}

/// Os menus de Layout (margens, orientação, tamanho, números de linha, alinhamento vertical) e o
/// de Inserir → Gráfico.
pub fn menu_pagina(app: &App, menu: MenuFaixa) -> Element<'static, Msg> {
    let doc = app.sessao.documento();
    let p = &doc.pagina;
    let ir = |c: Comando| Some(Msg::Comando(c));
    let mut col = column![].spacing(1);
    match menu {
        MenuFaixa::Margens => {
            for (nome, descricao, cm) in MARGENS {
                col = col.push(item_opcao(icone("borda_externa_espessa", 26.).into(), nome, descricao, margens_iguais(p, cm), ir(Comando::Margens(cm))));
            }
            col = col.push(pelicano::horizontal_rule(1.))
                .push(item_simples("settings", "Margens personalizadas…", Comando::Formulario(Form::Margens)));
            col = col.width(470);
        }
        MenuFaixa::Orientacao => {
            let paisagem = p.largura > p.altura;
            col = col
                .push(item_opcao(folha(17., 23.), "Retrato", "", !paisagem, ir(Comando::Orientacao(false))))
                .push(item_opcao(folha(23., 17.), "Paisagem", "", paisagem, ir(Comando::Orientacao(true))))
                .width(170);
        }
        MenuFaixa::Tamanho => {
            let papel = app.sessao.estado().papel;
            for (id, nome, medidas) in PAPEIS {
                // A folha desenhada na proporção do papel.
                let largura = match id { "carta" => 19., "oficio" => 15., _ => 17. };
                col = col.push(item_opcao(folha(largura, 24.), nome, medidas, papel == id, ir(Comando::Papel(id))));
            }
            col = col.push(pelicano::horizontal_rule(1.))
                .push(item_simples("settings", "Mais tamanhos de papel…", Comando::Formulario(Form::Papel)))
                .width(250);
        }
        MenuFaixa::NumerosLinha => {
            for (nome, id, v) in NUMEROS_DE_LINHA {
                col = col.push(item_marcado(nome, doc.extras.numerar_linhas == v, Comando::NumerarLinhas(id)));
            }
            col = col.width(220);
        }
        MenuFaixa::Vertical => {
            col = col.push(cabecalho_menu("Texto na página"));
            for (nome, id, ic, v) in VERTICAIS {
                col = col.push(item_opcao(icone(ic, 18.).into(), nome, "", doc.extras.vertical == v, ir(Comando::Vertical(id))));
            }
            col = col.width(200);
        }
        MenuFaixa::Graficos => {
            let em_tabela = app.visao.formato.tabela.is_some();
            col = col.push(cabecalho_menu(if em_tabela { "Gráfico com os dados da tabela" } else { "Clique numa tabela para criar o gráfico" }));
            for (t, nome, ic) in GRAFICOS {
                col = col.push(item_opcao(icone(ic, 22.).into(), nome, "", false, em_tabela.then(|| Msg::Comando(Comando::Grafico(t)))));
            }
            col = col.push(pelicano::horizontal_rule(1.)).push(item_opcao(icone("refresh", 18.).into(),
                "Atualizar gráfico selecionado", "Lê de novo a tabela de origem.", false,
                em_grafico(app).then_some(Msg::Comando(Comando::AtualizarGrafico)))).width(280);
        }
        _ => {}
    }
    col.into()
}

// ------------------------------------------------------------------ grupos e faixa estreita

use crate::faixa_responsiva::{planejar, Medida, Nivel};

/// Um grupo da faixa: o conteúdo inteiro e, se houver, o compacto, com as larguras de cada nível.
pub(crate) struct Grupo<'a> {
    nome: &'static str,
    icone: &'static str,
    medida: Medida,
    completo: Element<'a, Msg>,
    compacto: Option<Element<'a, Msg>>,
    /// O conteúdo já vem com a legenda (os grupos comuns da aba Desenhar).
    pronto: bool,
}
/// `largura`: a do grupo inteiro na tela, legenda e linha incluídas (medida na foto, pt-BR).
/// `prioridade`: menor encolhe antes.
fn grupo<'a>(nome: &'static str, icone: &'static str, largura: f32, prioridade: u8, completo: impl Into<Element<'a, Msg>>) -> Grupo<'a> {
    let nome = tr(nome);
    // O botão (ícone + setinha) e a legenda curta, o que for mais largo, mais o espaço do grupo.
    let recolhido = (curto_recolhido(nome).chars().count() as f32 * 5.3).max(53.) + 25.;
    Grupo { nome, icone, medida: Medida { completo: largura, compacto: None, recolhido, prioridade }, completo: completo.into(), compacto: None, pronto: false }
}
impl<'a> Grupo<'a> {
    fn compacto(mut self, largura: f32, conteudo: impl Into<Element<'a, Msg>>) -> Self {
        self.medida.compacto = Some(largura);
        self.compacto = Some(conteudo.into());
        self
    }
}

/// A legenda do grupo recolhido: a primeira palavra dos nomes longos ("Links e comentários" →
/// "Links"), como o "Transfer." do FeatherSheet; o nome inteiro fica na dica e no menu do grupo.
fn curto_recolhido(nome: &str) -> &str {
    if nome.chars().count() <= 12 { nome } else { nome.split(' ').next().unwrap_or(nome) }
}

/// A setinha do grupo compacto: abre o grupo inteiro num menu, como no FeatherSheet.
fn abrir_grupo(app: &App, i: u8, nome: &str) -> Element<'static, Msg> {
    let aberto = app.menu_faixa == Some(MenuFaixa::Grupo(i));
    botao_icone(icone("chevron-down", 12.), tr(nome), Some(Msg::MenuFaixa((!aberto).then_some(MenuFaixa::Grupo(i)))), aberto)
}
fn grande_seta(app: &App, nome_icone: &str, rotulo: &str, menu: MenuFaixa) -> Element<'static, Msg> {
    let aberto = app.menu_faixa == Some(menu);
    button(column![icone(nome_icone, 24.), row![text(tr(rotulo).to_string()).size(11).no_wrap(), icone("chevron-down", 10.)].spacing(3).align(Align::Center)]
        .spacing(3).align(Align::Center))
        .padding([4u16, 8])
        .style(estilo_botao_grande(aberto))
        .on_press(Msg::MenuFaixa((!aberto).then_some(menu)))
        .into()
}
fn pequeno_seta(app: &App, nome_icone: &str, rotulo: &str, menu: MenuFaixa) -> Element<'static, Msg> {
    let aberto = app.menu_faixa == Some(menu);
    button(row![icone(nome_icone, 16.), text(tr(rotulo).to_string()).size(11).no_wrap(), icone("chevron-down", 10.)].spacing(5).align(Align::Center))
        .padding([2u16, 6])
        .style(estilo_botao_pequeno(aberto))
        .on_press(Msg::MenuFaixa((!aberto).then_some(menu)))
        .into()
}
/// Botão só com ícone que fica apagado quando não vale (fora de uma tabela, por exemplo).
fn icone_se(nome: &str, dica: &str, cmd: Comando, disponivel: bool) -> Element<'static, Msg> {
    botao_icone(icone(nome, 16.), tr(dica), disponivel.then_some(Msg::Comando(cmd)), false)
}
fn cor(nome: &str, dica: &str, alvo: AlvoCor) -> Element<'static, Msg> {
    botao_icone(icone(nome, 16.), tr(dica), Some(Msg::AbrirCor(alvo)), false)
}

/// O campo de número do Layout → Parágrafo: o texto digitado fica enquanto ainda for o valor do
/// parágrafo (para "1," não virar "1" no meio da digitação).
fn campo(app: &App, qual: u8, valor: f32, unidade: &'static str, passo: f64) -> Element<'static, Msg> {
    let texto = match &app.campo_faixa {
        Some((q, t)) if *q == qual && feather_ui::campo_numero::ler(t).is_some_and(|v| (v - f64::from(valor)).abs() < 0.01) => t.clone(),
        _ => feather_ui::campo_numero::escrever((f64::from(valor) * 100.).round() / 100.),
    };
    row![
        feather_ui::campo_numero::campo_numero(&texto, "0", 70., feather_ui::campo_numero::Limites { passo, min: Some(0.), max: Some(200.) },
            false, move |t| Msg::Comando(Comando::CampoParagrafo(qual, t))),
        text(unidade).size(11).color(COR_TEXTO_MUTED()),
    ]
    .spacing(4)
    .align(Align::Center)
    .into()
}
/// O campo `qual` (0 recuo esquerdo e 1 direito, em cm; 2 espaço antes e 3 depois, em pt) mudou.
pub(crate) fn aplicar_campo(app: &mut App, qual: u8, texto: String) -> Result<(), String> {
    let Some(v) = feather_ui::campo_numero::ler(&texto) else {
        app.campo_faixa = Some((qual, texto));
        return Ok(());
    };
    let v = (v as f32).max(0.);
    let f = &app.visao.formato;
    let (recuo, dir, primeira) = (f.recuo, f.recuo_dir, f.primeira);
    app.campo_faixa = Some((qual, texto));
    match qual {
        0 => { app.sessao.recuos_definir(v * 72. / 2.54, dir, primeira); }
        1 => { app.sessao.recuos_definir(recuo, v * 72. / 2.54, primeira); }
        2 => { app.sessao.espaco_definir(Some(v), None)?; }
        _ => { app.sessao.espaco_definir(None, Some(v))?; }
    }
    Ok(())
}

fn grupos_inicio(app: &App) -> Vec<Grupo<'_>> {
    use featherdocs_core::modelo::{CODIGO, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
    let formato = &app.visao.formato;
    let fonte = |largura| feather_ui::formatacao::seletor_fonte(&["sans", "serifa", "mono"], Some(formato.fonte.as_str()), |s| Msg::Fonte(s.into()), largura);
    let tamanho = |largura| feather_ui::formatacao::seletor_tamanho(&[8u16, 10, 11, 12, 14, 16, 18, 24, 32, 48],
        Some(formato.tamanho.round() as u16), Msg::Tamanho, largura);
    let negrito = || so_icone("bold", "Negrito (Ctrl+B)", Comando::Formato(NEGRITO), formato.negrito);
    let italico = || so_icone("italic", "Itálico (Ctrl+I)", Comando::Formato(ITALICO), formato.italico);
    let sublinhado = || so_icone("underline", "Sublinhado (Ctrl+U)", Comando::Formato(SUBLINHADO), formato.sublinhado);
    let alinhamento = || feather_ui::alinhamento::seletor_horizontal(
        feather_ui::alinhamento::Horizontal::de_identificador(&formato.alinhamento), true, true,
        |escolha| Msg::Comando(Comando::Alinhar(match escolha {
            feather_ui::alinhamento::Horizontal::Esquerda => "esquerda",
            feather_ui::alinhamento::Horizontal::Centro => "centro",
            feather_ui::alinhamento::Horizontal::Direita => "direita",
            feather_ui::alinhamento::Horizontal::Justificado => "justificado",
        })));
    let colar = || grande("clipboard", "colar", Comando::Colar, false);
    let pincel = || so_icone("brush", "Pincel de formatação (Ctrl+Shift+C)", Comando::Pincel, formato.pincel);
    vec![
        grupo("area_transferencia", "clipboard", 239., 3, row![
            colar(),
            column![
                pequeno("cut", "recortar", Comando::Recortar, false),
                pequeno("copy", "copiar", Comando::Copiar, false),
                botao_pequeno(icone("brush", 16.0), tr("Pincel de formatação"), Some(Msg::Comando(Comando::Pincel)), formato.pincel),
            ]
            .spacing(5),
            column![so_icone("clipboard", "Colar sem formatação (Ctrl+Shift+V)", Comando::ColarSemFormatacao, false)],
        ].spacing(5))
        .compacto(128., row![
            colar(),
            column![
                so_icone("cut", "Recortar (Ctrl+X)", Comando::Recortar, false),
                so_icone("copy", "Copiar (Ctrl+C)", Comando::Copiar, false),
                pincel(),
            ],
        ].spacing(2).align(Align::Center)),
        grupo("fonte", "bold", 385., 9, column![
            row![
                fonte(110.),
                tamanho(65.),
                menu("Aa", &["Frase", "minúsculas", "MAIÚSCULAS", "Cada Palavra", "aLTERNAR"], |i| Comando::Maiusculas(MAIUSCULAS[i.min(4)].1), 105.0),
            ]
            .spacing(5),
            row![
                negrito(),
                italico(),
                sublinhado(),
                so_icone("strikethrough", "Tachado (Ctrl+Shift+X)", Comando::Formato(TACHADO), formato.tachado),
                so_icone("superscript", "Sobrescrito (Ctrl+Shift+=)", Comando::Posicao(1), formato.sobrescrito),
                so_icone("subscript", "Subscrito (Ctrl+=)", Comando::Posicao(-1), formato.subscrito),
                so_icone("code", "Código (Ctrl+Shift+E)", Comando::Formato(CODIGO), formato.codigo),
                so_icone("font-increase", "Aumentar fonte (Ctrl+Shift+>)", Comando::TamanhoPasso(1), false),
                so_icone("font-decrease", "Diminuir fonte (Ctrl+Shift+<)", Comando::TamanhoPasso(-1), false),
                cor("highlight", "Marca-texto", AlvoCor::Realce),
                cor("font-color", "Cor do texto", AlvoCor::Texto),
                so_icone("clear", "Limpar formatação (Ctrl+Espaço)", Comando::LimparFormatacao, false),
            ]
            .spacing(2)
            .align(Align::Center),
        ].spacing(7))
        .compacto(201., column![
            row![fonte(95.), tamanho(58.)].spacing(4),
            row![negrito(), italico(), sublinhado(), cor("highlight", "Marca-texto", AlvoCor::Realce),
                cor("font-color", "Cor do texto", AlvoCor::Texto), abrir_grupo(app, 1, "Mais opções de fonte")]
                .spacing(2).align(Align::Center),
        ].spacing(7)),
        grupo("paragrafo", "align-left", 263., 6, paragrafo(app))
        .compacto(182., column![
            row![
                so_icone("list", "Lista com marcadores (Ctrl+Shift+8)", Comando::Estilo("item"), formato.estilo == "item"),
                so_icone("list-numbers", "Lista numerada (Ctrl+Shift+7)", Comando::Estilo("item_ordenado"), formato.estilo == "item_ordenado"),
                so_icone("indent-decrease", "Diminuir recuo (Shift+Tab)", Comando::Recuar(-1), false),
                so_icone("indent-increase", "Aumentar recuo (Tab)", Comando::Recuar(1), false),
            ].spacing(2).align(Align::Center),
            row![alinhamento(), abrir_grupo(app, 2, "Mais opções de parágrafo")].spacing(2).align(Align::Center),
        ].spacing(8)),
        grupo("estilos", "palette", 435., 1, galeria_estilos(app, POR_LINHA))
            .compacto(243., galeria_estilos(app, 2)),
    ]
}

fn grupos_inserir(app: &App) -> Vec<Grupo<'_>> {
    let em_tabela = app.visao.formato.tabela.is_some();
    let bloco_selecionado = app.sessao.cursor().foco.bloco;
    let em_imagem = app.sessao.documento().blocos.iter().any(|b| {
        b.id == bloco_selecionado
            && matches!(&b.conteudo,
                featherdocs_core::modelo::Conteudo::Imagem { origem, .. }
                    if origem.starts_with("data:image/png;base64,")
                    || origem.starts_with("data:image/jpeg;base64,")
            )
    });
    let tipo = |i: usize| icone_se(GRAFICOS[i].2, GRAFICOS[i].1, Comando::Grafico(GRAFICOS[i].0), em_tabela);
    vec![
        grupo("objetos", "table", 283., 7, row![
            column![
                grande("table", "tabela", Comando::Tabela, false),
                menu("Tamanho", &["2 × 2", "4 × 4", "5 × 5", "6 × 6", "4 × 2", "8 × 4", "10 × 5", "Personalizada…"],
                    |i| match TABELAS.get(i) {
                        Some(&(_, l, c)) => Comando::TabelaTam(l, c),
                        None => Comando::Formulario(Form::Tabela),
                    }, 84.0),
            ]
            .spacing(2)
            .align(Align::Center),
            grande("photo", "imagem", Comando::Imagem, false),
            botao_pequeno(icone("wand", 16.), tr("Efeitos imagem"), em_imagem.then_some(Msg::AbrirEfeitosImagem), app.efeito_imagem.is_some()),
        ].spacing(6).align(Align::Center)),
        grupo("Gráficos", "chart-bar", 215., 6, row![
            grande_seta(app, "chart-bar", "Gráfico", MenuFaixa::Graficos),
            column![
                row![tipo(0), tipo(1), tipo(2), tipo(3)].spacing(2),
                row![tipo(4), tipo(5), tipo(6),
                    icone_se("refresh", "Atualizar o gráfico selecionado", Comando::AtualizarGrafico, em_grafico(app))].spacing(2),
            ].spacing(4),
        ].spacing(6).align(Align::Center)),
        grupo("paginas", "page-break", 243., 6, row![
            grande("page-break", "quebra", Comando::Quebra, false),
            column![
                botao_pequeno(icone("equacao", 16.), tr("Equação"), Some(Msg::Comando(Comando::Estilo("equacao"))), false),
                botao_pequeno(icone("minus", 16.), tr("Linha horizontal"), Some(Msg::Comando(Comando::Estilo("regua"))), false),
            ]
            .spacing(3),
        ].spacing(6))
        .compacto(150., column![
            pequeno("page-break", "quebra", Comando::Quebra, false),
            botao_pequeno(icone("equacao", 16.), tr("Equação"), Some(Msg::Comando(Comando::Estilo("equacao"))), false),
            botao_pequeno(icone("minus", 16.), tr("Linha horizontal"), Some(Msg::Comando(Comando::Estilo("regua"))), false),
        ].spacing(3)),
        grupo("Links e comentários", "link", 198., 5, row![
            grande("link", "link", Comando::Formulario(Form::Link), false),
            column![
                botao_pequeno(icone("comentario", 16.), tr("Comentário"), Some(Msg::FormAbrir(Form::Comentario)), false),
                botao_pequeno(icone("tag", 16.), tr("Marcador"), Some(Msg::FormAbrir(Form::Marcador)), false),
                botao_pequeno(icone("tag", 16.), tr("Referência cruzada"), Some(Msg::FormAbrir(Form::Referencia)), false),
            ]
            .spacing(3),
        ].spacing(5)),
        grupo("Cabeçalho e rodapé", "header-footer", 185., 5, column![
            row![
                botao_pequeno(icone("header-footer", 16.), tr("Cabeçalho"), Some(Msg::FormAbrir(Form::Cabecalho)), false),
                botao_pequeno(icone("header-footer", 16.), tr("Rodapé"), Some(Msg::FormAbrir(Form::Rodape)), false),
            ]
            .spacing(3),
            menu("Número de página", &NUMERO_PAGINA, numero_pagina, 150.0),
        ].spacing(4)),
        grupo("Editar tabela", "table", 180., 5, column![
            row![
                tabela_botao("Linha +", Comando::TabelaEstrutura("linha+"), em_tabela),
                tabela_botao("Linha −", Comando::TabelaEstrutura("linha-"), em_tabela),
            ].spacing(5),
            row![
                tabela_botao("Coluna +", Comando::TabelaEstrutura("coluna+"), em_tabela),
                tabela_botao("Coluna −", Comando::TabelaEstrutura("coluna-"), em_tabela),
            ].spacing(5),
            row![
                tabela_botao("Mesclar →", Comando::TabelaMesclar, em_tabela),
                tabela_botao("Desmesclar", Comando::TabelaDesmesclar, em_tabela),
            ].spacing(5),
        ].spacing(5)),
        grupo("Tabela", "table", 113., 5, column![
            {
                // Ligado aparece como botão ativo, sem caractere fazendo papel de ícone.
                let b = button(text(tr("Cabeçalho")).size(12)).padding([4, 8])
                    .style(estilo_botao_pequeno(app.sessao.tabela_tem_cabecalho()));
                if em_tabela { b.on_press(Msg::Comando(Comando::TabelaCabecalho)) } else { b }
            },
            tabela_botao("Excluir tabela", Comando::TabelaExcluir, em_tabela),
        ].spacing(5)),
    ]
}

fn grupos_layout(app: &App) -> Vec<Grupo<'_>> {
    let f = &app.visao.formato;
    let rotulo = |t: &str| text(tr(t).to_string()).size(11).width(58);
    vec![
        grupo("Configurar página", "borda_externa_espessa", 409., 5, row![
            grande_seta(app, "borda_externa_espessa", "Margens", MenuFaixa::Margens),
            grande_seta(app, "orientacao_pagina", "Orientação", MenuFaixa::Orientacao),
            grande_seta(app, "arrows-maximize", "Tamanho", MenuFaixa::Tamanho),
            column![
                pequeno_seta(app, "quebra_pagina", "Quebras", MenuFaixa::Quebras),
                pequeno_seta(app, "list-numbers", "Números de linha", MenuFaixa::NumerosLinha),
                pequeno_seta(app, "layout-align-middle", "Alinhamento vertical", MenuFaixa::Vertical),
            ].spacing(3),
        ].spacing(4).align(Align::Center)),
        grupo("paragrafo", "indent-increase", 338., 5, row![
            column![
                text(tr("Recuo").to_string()).size(11).font(Font::SEMIBOLD),
                row![rotulo("Esquerda:"), campo(app, 0, f.recuo / 72. * 2.54, "cm", 0.25)].spacing(4).align(Align::Center),
                row![rotulo("Direita:"), campo(app, 1, f.recuo_dir / 72. * 2.54, "cm", 0.25)].spacing(4).align(Align::Center),
            ].spacing(4),
            column![
                text(tr("Espaçamento").to_string()).size(11).font(Font::SEMIBOLD),
                row![rotulo("Antes:"), campo(app, 2, f.antes, "pt", 6.)].spacing(4).align(Align::Center),
                row![rotulo("Depois:"), campo(app, 3, f.depois, "pt", 6.)].spacing(4).align(Align::Center),
            ].spacing(4),
        ].spacing(14)),
        grupo("Cabeçalho e rodapé", "header-footer", 179., 5, column![
            botao_pequeno(icone("header-footer", 16.), tr("Primeira página diferente"),
                Some(Msg::Comando(Comando::PrimeiraDiferente)), app.sessao.documento().extras.primeira_diferente),
            botao_pequeno(icone("header-footer", 16.), tr("Cabeçalho"), Some(Msg::FormAbrir(Form::Cabecalho)), false),
            botao_pequeno(icone("header-footer", 16.), tr("Rodapé"), Some(Msg::FormAbrir(Form::Rodape)), false),
        ].spacing(3)),
    ]
}

fn grupos_desenhar(app: &App) -> Vec<Grupo<'static>> {
    // Os grupos comuns da suíte (os mesmos do FeatherSheet), já com a legenda.
    const NOMES: [(&str, &str); 3] = [("Ferramentas de desenho", "pencil"), ("Converter", "wand"), ("Modo", "hand-stop")];
    feather_ui::tinta::grupos_faixa(&app.tinta.ui).into_iter().zip(NOMES).zip(feather_ui::tinta::LARGURAS_GRUPOS)
        .map(|((e, (nome, ic)), largura)| Grupo { pronto: true, ..grupo(nome, ic, largura, 5, e.map(Msg::Desenhar)) })
        .collect()
}

fn grupos_referencias(_app: &App) -> Vec<Grupo<'static>> {
    vec![
        grupo("Sumário", "list-details", 202., 5, row![
            grande("list-details", "sumario", Comando::Sumario, false),
            column![
                menu("Sumário", &["Até o nível 2", "Até o nível 3", "Até o nível 4", "Remover sumários"],
                    |i| match i {
                        3 => Comando::SumarioRemover,
                        _ => Comando::SumarioNiveis(i as u8 + 2),
                    }, 100.0),
                botao_pequeno(icone("refresh", 16.), tr("Atualizar sumário"), Some(Msg::Comando(Comando::ReferenciasAtualizar)), false),
            ]
            .spacing(4),
        ].spacing(5)),
        grupo("Notas de rodapé", "text-box", 164., 5, column![
            botao_pequeno(icone("text-box", 16.), tr("Inserir nota de rodapé"), Some(Msg::FormAbrir(Form::NotaRodape)), false),
            botao_pequeno(icone("text-box", 16.), tr("Inserir nota de fim"), Some(Msg::FormAbrir(Form::NotaFim)), false),
        ].spacing(3)),
        grupo("Citações e bibliografia", "message-circle", 223., 5, column![
            row![
                botao_pequeno(icone("plus", 16.), tr("Nova fonte"), Some(Msg::FormAbrir(Form::NovaFonte)), false),
                botao_pequeno(icone("file-import", 16.), tr("Importar .bib"), Some(Msg::Comando(Comando::ImportarBib)), false),
            ]
            .spacing(3),
            botao_pequeno(icone("message-circle", 16.), tr("Inserir citação"), Some(Msg::FormAbrir(Form::Citacao)), false),
            row![
                menu("Bibliografia", &["Só as citadas", "Todas as fontes"], |i| Comando::Bibliografia(i == 1), 100.0),
                menu("Estilo", &["ABNT", "APA", "Vancouver", "IEEE", "Chicago"],
                    |i| Comando::EstiloCitacao(["abnt", "apa", "vancouver", "ieee", "chicago"][i]), 92.0),
            ]
            .spacing(4),
        ].spacing(4)),
        grupo("Legendas", "tag", 177., 5, column![
            menu("Inserir legenda", &["Figura", "Tabela", "Equação"],
                |i| Comando::Legenda(["figura", "tabela", "equacao"][i]), 150.0),
            menu("Índice de ilustrações", &["Lista de figuras", "Lista de tabelas"],
                |i| Comando::ListaDe(i as u8 + 1), 150.0),
            botao_pequeno(icone("tag", 16.), tr("Referência cruzada"), Some(Msg::FormAbrir(Form::Referencia)), false),
        ].spacing(4)),
        grupo("Índice", "list-details", 133., 5, column![
            botao_pequeno(icone("list-details", 16.), tr("Marcar entrada"), Some(Msg::FormAbrir(Form::IndiceEntrada)), false),
            botao_pequeno(icone("list-details", 16.), tr("Inserir índice"), Some(Msg::Comando(Comando::IndiceInserir)), false),
        ].spacing(3)),
    ]
}

fn grupos_revisao(app: &App) -> Vec<Grupo<'_>> {
    let modo = app.sessao.modo_edicao();
    vec![
        grupo("busca", "search", 85., 5, row![grande("search", "localizar", Comando::Buscar, false)]),
        grupo("modo", "edit", 171., 5, row![
            grande("edit", "editar", Comando::Modo("editando"), modo == "editando"),
            grande("comentario", "sugerir", Comando::Modo("sugerindo"), modo == "sugerindo"),
            grande("eye", "leitura", Comando::Modo("leitura"), modo == "leitura"),
        ]),
        grupo("alteracoes", "checks", 272., 5, row![
            botao_grande(icone("arrow-forward-up", 24.), tr("Aceitar"), Some(Msg::Comando(Comando::Revisoes(true, false))), false),
            botao_grande(icone("arrow-back-up", 24.), tr("Rejeitar"), Some(Msg::Comando(Comando::Revisoes(false, false))), false),
            column![
                botao_pequeno(icone("checks", 16.), tr("Aceitar todas"), Some(Msg::Comando(Comando::Revisoes(true, true))), false),
                botao_pequeno(icone("x", 16.), tr("Rejeitar todas"), Some(Msg::Comando(Comando::Revisoes(false, true))), false),
                botao_pequeno(icone("columns", 16.), tr("Comparar com arquivo"), Some(Msg::Comando(Comando::Comparar)), false),
            ]
            .spacing(3),
        ]),
        grupo("Comentários", "comentario", 170., 5, row![
            grande("comentario", "novo_comentario", Comando::Formulario(Form::Comentario), false),
            grande("layout-sidebar", "painel_revisao", Comando::PainelRevisao, app.painel_revisao()),
        ]),
        grupo("Proteger", "lock", 84., 5, row![
            grande("lock", "proteger", Comando::Proteger, app.sessao.documento().extras.protegido),
        ]),
    ]
}

fn grupos_exibir(app: &App) -> Vec<Grupo<'_>> {
    vec![
        grupo("paineis", "layout-sidebar", 252., 5, row![
            grande("layout-sidebar", "estrutura", Comando::Estrutura, app.config.estrutura),
            grande("list-details", "marcas", Comando::Marcas, app.sessao.estado().marcas),
            grande("code", "codigo_latex", Comando::Codigo(Fonte::Latex), app.fonte_do_codigo() == Some(Fonte::Latex)),
            grande("file-code", "codigo_markdown", Comando::Codigo(Fonte::Markdown), app.fonte_do_codigo() == Some(Fonte::Markdown)),
        ]),
        grupo("zoom", "zoom-in", 133., 5, row![
            grande("zoom-out", "reduzir", Comando::Zoom(-0.1), false),
            grande("zoom-in", "ampliar", Comando::Zoom(0.1), false),
        ]),
        grupo("preferencias", "settings", 112., 5, row![grande("settings", "configuracoes", Comando::Configuracoes, false)]),
    ]
}

/// Os grupos da aba, na ordem da tela.
pub(crate) fn grupos(app: &App, aba: Aba) -> Vec<Grupo<'_>> {
    match aba {
        Aba::Inicio => grupos_inicio(app),
        Aba::Inserir => grupos_inserir(app),
        Aba::Desenhar => grupos_desenhar(app),
        Aba::Design => vec![grupo("Plano de fundo da página", "bucket", 147., 5, column![
            botao_pequeno(icone("bucket", 16.), tr("Cor da página"), Some(Msg::AbrirCor(AlvoCor::Pagina)), false),
            botao_pequeno(icone("square", 16.), tr("Borda da página"), Some(Msg::FormAbrir(Form::BordaPagina)), false),
            botao_pequeno(icone("text-box", 16.), tr("Marca d'água"), Some(Msg::FormAbrir(Form::MarcaDagua)), false),
        ].spacing(3))],
        Aba::Layout => grupos_layout(app),
        Aba::Referencias => grupos_referencias(app),
        Aba::Correspondencias => vec![
            grupo("Criar", "text-box", 153., 5, row![em_breve("text-box", "Envelopes"), em_breve("tag", "Etiquetas")]),
            grupo("Mala direta", "forms", 206., 5, row![em_breve("forms", "Iniciar mala direta"), em_breve("user", "Destinatários")]),
        ],
        Aba::Revisao => grupos_revisao(app),
        Aba::Exibir => grupos_exibir(app),
    }
}

/// Os níveis dos grupos da aba aberta para a largura da janela.
fn niveis(app: &App, grupos: &[Grupo<'_>]) -> Vec<Nivel> {
    let medidas: Vec<Medida> = grupos.iter().map(|g| g.medida).collect();
    // A faixa tem 8 px de cada lado.
    planejar(&medidas, app.largura_janela() - 16.)
}

/// Um grupo recolhido do menu `Grupo(i)` aberto inteiro, com o nome em cima.
pub fn menu_grupo(app: &App, i: u8) -> Element<'_, Msg> {
    match grupos(app, app.aba).into_iter().nth(usize::from(i)) {
        Some(g) => column![cabecalho_menu(g.nome), container(g.completo).padding([8, 10])].spacing(2).into(),
        None => Space::new(0, 0).into(),
    }
}

/// "Recolher faixa" / "Expandir faixa" (Ctrl+F1), na ponta direita da linha das abas.
pub(crate) fn botao_recolher(app: &App) -> Element<'static, Msg> {
    button(text(tr(if app.config.faixa_recolhida { "Expandir faixa" } else { "Recolher faixa" }).to_string()).size(12))
        .padding([4, 8])
        .style(estilo_botao_pequeno(false))
        .on_press(Msg::AlternarFaixa)
        .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    if app.config.faixa_recolhida {
        return crate::menu_classico::barra(app);
    }
    // Modelo de abas da suíte; enum Aba e Msg continuam do FeatherDocs.
    let descritores: Vec<_> = Aba::TODAS.iter().copied()
        .map(|aba| feather_ui::ribbon::AbaRibbon::nova(aba, aba.nome()))
        .collect();
    let abas = feather_ui::ribbon::abas(&descritores, &app.aba, Msg::Aba);
    let grupos = grupos(app, app.aba);
    let niveis = niveis(app, &grupos);
    let conteudo = row(grupos.into_iter().zip(niveis).enumerate().map(|(i, (g, n))| match n {
        Nivel::Completo | Nivel::Compacto if g.pronto => g.completo,
        Nivel::Completo => grupo_faixa(g.completo, g.nome),
        Nivel::Compacto => grupo_faixa(g.compacto.unwrap_or(g.completo), g.nome),
        Nivel::Recolhido => {
            let i = i as u8;
            let aberto = app.menu_faixa == Some(MenuFaixa::Grupo(i));
            let botao = button(column![icone(g.icone, 21.), icone("chevron-down", 11.)].spacing(7).align(Align::Center))
                .padding([10, 16])
                .style(estilo_botao_grande(aberto))
                .on_press(Msg::MenuFaixa((!aberto).then_some(MenuFaixa::Grupo(i))));
            grupo_faixa(feather_ui::formatacao::dica(botao, g.nome), curto_recolhido(g.nome))
        }
    }));
    column![
        row![abas, horizontal_space(), botao_recolher(app)].align(Align::Center).padding(Padding { right: 8., ..Padding::ZERO }),
        // Rolagem só de reserva: abaixo de ~560 px nem tudo recolhido cabe na Inserir.
        scrollable(container(conteudo).height(106u16))
            .direction(pelicano::Direction::Horizontal)
            // A rolagem do pelicano enche a altura por padrão (a do Iced encolhia ao conteúdo).
            .height(Length::Shrink)
            .scrollbar_width(6u16)
            .scrollbar_spacing(2u16)
    ]
    .spacing(0)
    .width(Length::FILL)
    .align(Align::Start)
    .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Em qualquer largura e aba a faixa cabe (até 640 px; abaixo de ~560 a Inserir rola) e todo grupo recolhido abre o menu dele.
    #[test]
    fn toda_aba_cabe_na_janela_estreita() {
        let mut app = App::default();
        for aba in Aba::TODAS {
            app.aba = aba;
            let g = grupos(&app, aba);
            let medidas: Vec<Medida> = g.iter().map(|x| x.medida).collect();
            for largura in [1920., 1280., 1024., 800., 640.] {
                let n = planejar(&medidas, largura - 16.);
                assert!(crate::faixa_responsiva::largura_total(&medidas, &n) <= largura - 16., "{aba:?} em {largura}px");
            }
            let total = g.len() as u8;
            drop(g);
            for i in 0..total {
                app.menu_faixa = Some(MenuFaixa::Grupo(i));
                let _ = menu_grupo(&app, i);
            }
        }
        // A Página inicial passa por compacto antes de recolher qualquer grupo.
        app.aba = Aba::Inicio;
        let g = grupos(&app, Aba::Inicio);
        let medidas: Vec<Medida> = g.iter().map(|x| x.medida).collect();
        let total_compacto: f32 = medidas.iter().map(|m| m.compacto.unwrap_or(m.completo)).sum();
        assert!(planejar(&medidas, total_compacto).iter().all(|&n| n == Nivel::Compacto));
    }

    #[test]
    fn campos_do_paragrafo_mudam_recuo_e_espaco() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::CampoParagrafo(0, "1,5".into())));
        assert!((app.visao.formato.recuo - 1.5 * 72. / 2.54).abs() < 0.1);
        // "2," no meio da digitação fica como foi escrito, e já vale 2 cm.
        let _ = app.update(Msg::Comando(Comando::CampoParagrafo(0, "2,".into())));
        assert!((app.visao.formato.recuo - 2. * 72. / 2.54).abs() < 0.1);
        let _ = app.update(Msg::Comando(Comando::CampoParagrafo(3, "12".into())));
        assert!((app.visao.formato.depois - 12.).abs() < 0.01);
        let _ = app.update(Msg::Comando(Comando::CampoParagrafo(2, "abc".into())));
        assert!((app.visao.formato.antes).abs() < 0.01 || app.visao.formato.antes > 0.);
    }
}
