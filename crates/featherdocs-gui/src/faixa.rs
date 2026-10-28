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
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{font, Alignment, Element, Font, Length};

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
    iced::widget::tooltip(
        botao_grande(icone(nome, 24.), tr(rotulo), None, false),
        container(text(tr("Em breve")).size(11)).padding([3, 6]).style(estilo_dica),
        iced::widget::tooltip::Position::Bottom,
    )
    .into()
}
fn estilo_dica(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(COR_PAINEL().into()),
        border: iced::Border { color: COR_BORDA(), width: 1., radius: 4.into() },
        text_color: Some(COR_TEXTO()),
        ..Default::default()
    }
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
}

/// A letra da amostra: a da página (Liberation), não a da interface.
fn fonte_da_amostra(negrito: bool, italico: bool, mono: bool) -> Font {
    Font {
        family: font::Family::Name(if mono { "Liberation Mono" } else { "Liberation Sans" }),
        weight: if negrito { font::Weight::Bold } else { font::Weight::Normal },
        style: if italico { font::Style::Italic } else { font::Style::Normal },
        ..Font::DEFAULT
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
        .wrapping(iced::widget::text::Wrapping::None);
    let amostra = if cor == 0 { amostra } else { amostra.color(crate::pagina::cor(cor)) };
    let rotulo = text(curto(&nome, 12)).size(10.5).color(COR_TEXTO_MUTED()).wrapping(iced::widget::text::Wrapping::None);
    let dica = nome.clone();
    let bloco = button(
        column![
            container(amostra).center_x(Length::Fill).height(Length::Fill).align_y(Alignment::Center).clip(true),
            container(rotulo).center_x(Length::Fill).clip(true),
        ]
        .spacing(1),
    )
    .width(BLOCO.0)
    .height(BLOCO.1)
    .padding([4, 4])
    .on_press(msg)
    .style(move |_, status| button::Style {
        background: Some(if status == button::Status::Hovered { COR_HOVER() } else { COR_PAINEL() }.into()),
        text_color: COR_TEXTO(),
        border: iced::Border {
            color: if ativo { COR_AZUL_ACENTO() } else { COR_BORDA() },
            width: if ativo { 2. } else { 1. },
            radius: 4.into(),
        },
        ..Default::default()
    });
    iced::widget::tooltip(bloco, text(dica).size(11), iced::widget::tooltip::Position::Bottom)
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
/// A galeria de estilos no jeito do Word: quatro por vez, setas para rolar e a de abrir a galeria inteira.
fn galeria_estilos(app: &App) -> Element<'_, Msg> {
    let blocos = blocos_de_estilo(app);
    let inicio = app.galeria_estilos.min(blocos.len().saturating_sub(POR_LINHA));
    let fim = inicio + POR_LINHA;
    let total = blocos.len();
    let visiveis: Vec<_> = blocos.into_iter().skip(inicio).take(POR_LINHA).collect();
    let seta = |nome: &str, dica: &str, msg: Option<Msg>| botao_icone(icone(nome, 12.), tr(dica), msg, false);
    let aberta = app.menu_faixa == Some(MenuFaixa::Estilos);
    column![
        row![
            iced::widget::Row::with_children(visiveis).spacing(4),
            column![
                seta("chevron-up", "Estilos anteriores", (inicio > 0).then_some(Msg::GaleriaEstilos(-(POR_LINHA as i32)))),
                seta("chevron-down", "Mais estilos", (fim < total).then_some(Msg::GaleriaEstilos(POR_LINHA as i32))),
                botao_icone(icone("layout-grid", 12.), tr("Todos os estilos"),
                    Some(Msg::MenuFaixa((!aberta).then_some(MenuFaixa::Estilos))), aberta),
            ]
            .spacing(1),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
        row![
            pequeno("plus", "novo_estilo", Comando::Formulario(Form::NovoEstilo), false),
            pequeno("refresh", "atualizar_estilo", Comando::EstiloAtualizar, false),
            pequeno("trash", "excluir_estilo", Comando::EstiloExcluir, false),
        ]
        .spacing(4),
    ]
    .spacing(4)
    .into()
}

/// O título de uma parte do menu, numa faixa cinza (como "Quebras de Página" no Word).
fn cabecalho_menu(titulo: &str) -> Element<'static, Msg> {
    container(text(tr(titulo).to_string()).size(11).font(Font { weight: font::Weight::Semibold, ..Font::DEFAULT }))
        .width(Length::Fill)
        .padding([4, 10])
        .style(|_| container::Style { background: Some(COR_HOVER().into()), text_color: Some(COR_TEXTO()), ..Default::default() })
        .into()
}
/// Um item de menu com ícone grande, nome e a explicação embaixo.
fn item_descrito(nome_icone: &str, titulo: &str, descricao: &str, cmd: Comando) -> Element<'static, Msg> {
    button(
        row![
            icone(nome_icone, 26.),
            column![
                text(tr(titulo).to_string()).size(13).font(Font { weight: font::Weight::Semibold, ..Font::DEFAULT }),
                text(tr(descricao).to_string()).size(11).color(COR_TEXTO_MUTED()),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([6, 10])
    .style(estilo_botao_pequeno(false))
    .on_press(Msg::Comando(cmd))
    .into()
}
/// Um item de menu de uma linha só.
fn item_simples(nome_icone: &str, titulo: &str, cmd: Comando) -> Element<'static, Msg> {
    button(row![icone(nome_icone, 16.), text(tr(titulo).to_string()).size(12)].spacing(10).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([5, 10])
        .style(estilo_botao_pequeno(false))
        .on_press(Msg::Comando(cmd))
        .into()
}

/// Botão que abre um menu da faixa: o ícone e a setinha; clicar de novo fecha.
fn botao_de_menu(app: &App, nome_icone: &str, dica: &str, menu: MenuFaixa) -> Element<'static, Msg> {
    let aberto = app.menu_faixa == Some(menu);
    feather_ui::formatacao::dica(
        button(row![icone(nome_icone, 16.), icone("chevron-down", 10.)].spacing(1).align_y(Alignment::Center))
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
    let separador = || container(iced::widget::Space::new(1, 18)).style(|_| container::Style { background: Some(COR_BORDA().into()), ..Default::default() });
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
        .align_y(Alignment::Center),
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
        .align_y(Alignment::Center),
    ]
    .spacing(8)
    .into()
}

/// Um item de menu com a marca de escolhido à esquerda.
fn item_marcado(titulo: &str, marcado: bool, cmd: Comando) -> Element<'static, Msg> {
    let marca: Element<'static, Msg> = if marcado { icone("check", 14.).into() } else { iced::widget::Space::new(14, 14).into() };
    button(row![marca, text(tr(titulo).to_string()).size(12)].spacing(10).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([5, 10])
        .style(estilo_botao_pequeno(false))
        .on_press(Msg::Comando(cmd))
        .into()
}

/// Os menus pequenos do grupo Parágrafo (marcadores, numeração, espaçamento, bordas).
pub fn menu_paragrafo(app: &App, menu: MenuFaixa) -> Element<'static, Msg> {
    let f = &app.visao.formato;
    let mut col = column![].spacing(1).width(240);
    match menu {
        MenuFaixa::Marcadores => {
            col = col.push(cabecalho_menu("Biblioteca de marcadores"));
            for (nome, id) in [("Disco", "padrao"), ("Círculo", "circulo"), ("Quadrado", "quadrado"), ("Seta", "seta"), ("Caixa de tarefa", "tarefa")] {
                col = col.push(item_marcado(nome, f.estilo == "item" && f.lista == id, Comando::Lista(id)));
            }
        }
        MenuFaixa::Numeracao => {
            col = col.push(cabecalho_menu("Biblioteca de numeração"));
            for (nome, id) in [("1. 2. 3.", "numerica"), ("a. b. c.", "alfabetica"), ("i. ii. iii.", "romana")] {
                col = col.push(item_marcado(nome, f.estilo == "item_ordenado" && f.lista == id, Comando::Lista(id)));
            }
            col = col.push(iced::widget::horizontal_rule(1)).push(item_marcado("Reiniciar numeração em 1", false, Comando::ListaReiniciar));
        }
        MenuFaixa::Entrelinha => {
            col = col.push(cabecalho_menu("Entrelinha"));
            for (nome, v) in [("1,0", 1.0), ("1,15", 1.15), ("1,5", 1.5), ("2,0", 2.0), ("2,5", 2.5), ("3,0", 3.0)] {
                col = col.push(item_marcado(nome, (f.entrelinha - v).abs() < 0.01, Comando::Entrelinha(v)));
            }
            col = col.push(item_marcado("Padrão", f.entrelinha == 0., Comando::Entrelinha(0.)));
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
            for (nome, bits) in [("Sem borda", 0u8), ("Borda inferior", 8), ("Borda superior", 4), ("Borda esquerda", 1), ("Borda direita", 2), ("Contorno", 15)] {
                col = col.push(item_marcado(nome, f.borda == bits, Comando::Borda(bits)));
            }
        }
        MenuFaixa::Quebras | MenuFaixa::Estilos => {}
    }
    col.into()
}

/// Layout → Quebras: as de página e as de seção, como no Word.
pub fn menu_quebras() -> Element<'static, Msg> {
    column![
        cabecalho_menu("Quebras de página"),
        item_descrito("page-break", "Página", "Marcar o ponto em que uma página termina e a próxima começa.", Comando::Quebra),
        item_descrito("columns", "Coluna", "Indicar que o texto após a quebra de coluna começará na coluna seguinte.", Comando::Estilo("quebra_de_coluna")),
        item_descrito("text-wrap", "Disposição do texto", "Separar o texto ao redor dos objetos, como o texto das legendas do corpo do texto.", Comando::QuebraDeLinha),
        cabecalho_menu("Quebras de seção"),
        item_descrito("file-plus", "Próxima página", "Inserir uma quebra de seção e iniciar a nova seção na página seguinte.", Comando::Estilo("secao_proxima")),
        item_descrito("arrows-vertical", "Contínuo", "Inserir uma quebra de seção e iniciar a nova seção na mesma página.", Comando::Estilo("secao_continua")),
        item_descrito("copy", "Página par", "Inserir uma quebra de seção e iniciar a nova seção na próxima página par.", Comando::Estilo("secao_par")),
        item_descrito("copy-plus", "Página ímpar", "Inserir uma quebra de seção e iniciar a nova seção na próxima página ímpar.", Comando::Estilo("secao_impar")),
    ]
    .spacing(1)
    .width(400)
    .into()
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
        grade = grade.push(iced::widget::Row::with_children(linha).spacing(4));
    }
    column![
        container(scrollable(grade).height(Length::Shrink)).padding(8).max_height(320),
        iced::widget::horizontal_rule(1),
        item_simples("plus", "Salvar seleção como novo estilo rápido…", Comando::Formulario(Form::NovoEstilo)),
        item_simples("clear", "Limpar formatação", Comando::LimparFormatacao),
        item_simples("wand", "Aplicar estilos…", Comando::Formulario(Form::AplicarEstilo)),
    ]
    .spacing(2)
    .width(LARGURA_GALERIA_ABERTA)
    .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let formato = &app.visao.formato;
    let em_tabela = formato.tabela.is_some();
    let bloco_selecionado = app.sessao.cursor().foco.bloco;
    let em_imagem = app.sessao.documento().blocos.iter().any(|b| {
        b.id == bloco_selecionado
            && matches!(&b.conteudo,
                featherdocs_core::modelo::Conteudo::Imagem { origem, .. }
                    if origem.starts_with("data:image/png;base64,")
                    || origem.starts_with("data:image/jpeg;base64,")
            )
    });
    let em_grafico = app.sessao.documento().blocos.iter().any(|b| {
        b.id == bloco_selecionado && matches!(
            &b.conteudo,
            featherdocs_core::modelo::Conteudo::Imagem { origem, .. }
                if featherdocs_io::grafico_docs::eh_grafico(origem)
        )
    });
    // Modelo de abas da suíte; enum Aba e Msg continuam do FeatherDocs.
    let descritores: Vec<_> = Aba::TODAS.iter().copied()
        .map(|aba| feather_ui::ribbon::AbaRibbon::nova(aba, aba.nome()))
        .collect();
    let abas = feather_ui::ribbon::abas(&descritores, &app.aba, Msg::Aba);
    let conteudo: Element<'_, Msg> = match app.aba {
        Aba::Inicio => row![
            grupo_faixa(
                row![
                    grande("clipboard", "colar", Comando::Colar, false),
                    column![
                        pequeno("cut", "recortar", Comando::Recortar, false),
                        pequeno("copy", "copiar", Comando::Copiar, false),
                        botao_pequeno(
                            icone("brush", 16.0),
                            tr("Pincel de formatação"),
                            Some(Msg::Comando(Comando::Pincel)),
                            formato.pincel,
                        )
                    ]
                    .spacing(5),
                    column![so_icone("clipboard", "Colar sem formatação (Ctrl+Shift+V)", Comando::ColarSemFormatacao, false)]
                ]
                .spacing(5)
                .into(),
                tr("area_transferencia")
            ),
            grupo_faixa(
                column![
                    row![
                        feather_ui::formatacao::seletor_fonte(
                            &["sans", "serifa", "mono"],
                            Some(formato.fonte.as_str()), |s| Msg::Fonte(s.into()), 110.0
                        ),
                        feather_ui::formatacao::seletor_tamanho(
                            &[8u16, 10, 11, 12, 14, 16, 18, 24, 32, 48],
                            Some(formato.tamanho.round() as u16), Msg::Tamanho, 65.0
                        ),
                        menu("Aa", &["Frase", "minúsculas", "MAIÚSCULAS", "Cada Palavra", "aLTERNAR"],
                            |i| Comando::Maiusculas(["frase", "minusculas", "maiusculas", "palavras", "alternar"][i]), 105.0),
                    ]
                    .spacing(5),
                    row![
                        so_icone("bold", "Negrito (Ctrl+B)", Comando::Formato(featherdocs_core::modelo::NEGRITO), formato.negrito),
                        so_icone("italic", "Itálico (Ctrl+I)", Comando::Formato(featherdocs_core::modelo::ITALICO), formato.italico),
                        so_icone("underline", "Sublinhado (Ctrl+U)", Comando::Formato(featherdocs_core::modelo::SUBLINHADO), formato.sublinhado),
                        so_icone("strikethrough", "Tachado (Ctrl+Shift+X)", Comando::Formato(featherdocs_core::modelo::TACHADO), formato.tachado),
                        so_icone("superscript", "Sobrescrito (Ctrl+Shift+=)", Comando::Posicao(1), formato.sobrescrito),
                        so_icone("subscript", "Subscrito (Ctrl+=)", Comando::Posicao(-1), formato.subscrito),
                        so_icone("code", "Código (Ctrl+Shift+E)", Comando::Formato(featherdocs_core::modelo::CODIGO), formato.codigo),
                        so_icone("font-increase", "Aumentar fonte (Ctrl+Shift+>)", Comando::TamanhoPasso(1), false),
                        so_icone("font-decrease", "Diminuir fonte (Ctrl+Shift+<)", Comando::TamanhoPasso(-1), false),
                        so_icone("clear", "Limpar formatação (Ctrl+Espaço)", Comando::LimparFormatacao, false),
                    ]
                    .spacing(2)
                    .align_y(Alignment::Center)
                ]
                .spacing(7)
                .into(),
                tr("fonte")
            ),
            grupo_faixa(
                column![
                    button(text(tr("Cor do texto")).size(12))
                        .on_press(Msg::AbrirCor(AlvoCor::Texto))
                        .style(estilo_botao_pequeno(false)),
                    button(text(tr("Marca-texto")).size(12))
                        .on_press(Msg::AbrirCor(AlvoCor::Realce))
                        .style(estilo_botao_pequeno(false)),
                ]
                .spacing(3)
                .into(),
                tr("Cores")
            ),
            grupo_faixa(paragrafo(app), tr("paragrafo")),
            grupo_faixa(
                galeria_estilos(app),
                tr("estilos")
            ),
        ]
        .into(),
        Aba::Inserir => row![
            grupo_faixa(
                row![
                    column![
                        grande("table", "tabela", Comando::Tabela, false),
                        menu("Tamanho", &["2 × 2", "4 × 4", "5 × 5", "6 × 6", "4 × 2", "8 × 4", "10 × 5", "Personalizada…"],
                            |i| match i {
                                7 => Comando::Formulario(Form::Tabela),
                                _ => {
                                    let (l, c) = [(2, 2), (4, 4), (5, 5), (6, 6), (4, 2), (8, 4), (10, 5)][i];
                                    Comando::TabelaTam(l, c)
                                }
                            }, 84.0),
                    ]
                    .spacing(2)
                    .align_x(Alignment::Center),
                    grande("photo", "imagem", Comando::Imagem, false),
                    button(text(tr("Efeitos imagem")).size(12))
                        .on_press_maybe(em_imagem.then_some(Msg::AbrirEfeitosImagem))
                        .style(estilo_botao_pequeno(app.efeito_imagem.is_some()))
                ]
                .spacing(6)
                .into(),
                tr("objetos")
            ),
            grupo_faixa(
                column![
                    row![
                        tabela_botao("Colunas", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Colunas), em_tabela),
                        tabela_botao("Barras", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Barras), em_tabela),
                        tabela_botao("Linhas", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Linhas), em_tabela),
                    ].spacing(4),
                    row![
                        tabela_botao("Área", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Area), em_tabela),
                        tabela_botao("Pizza", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Pizza), em_tabela),
                        tabela_botao("Dispersão", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Dispersao), em_tabela),
                    ].spacing(4),
                    row![
                        tabela_botao("Degraus", Comando::Grafico(featherdocs_io::grafico_docs::Tipo::Degraus), em_tabela),
                        tabela_botao("Atualizar selecionado", Comando::AtualizarGrafico, em_grafico),
                    ].spacing(4),
                ].spacing(5).into(),
                tr("Gráficos (Charts)")
            ),
            grupo_faixa(
                row![
                    grande("page-break", "quebra", Comando::Quebra, false),
                    column![
                        botao_pequeno(icone("equacao", 16.), tr("Equação"), Some(Msg::Comando(Comando::Estilo("equacao"))), false),
                        botao_pequeno(icone("minus", 16.), tr("Linha horizontal"), Some(Msg::Comando(Comando::Estilo("regua"))), false),
                    ]
                    .spacing(3)
                ]
                .spacing(6)
                .into(),
                tr("paginas")
            ),
            grupo_faixa(
                row![
                    grande("link", "link", Comando::Formulario(Form::Link), false),
                    column![
                        botao_pequeno(icone("comentario", 16.), tr("Comentário"), Some(Msg::FormAbrir(Form::Comentario)), false),
                        botao_pequeno(icone("tag", 16.), tr("Marcador"), Some(Msg::FormAbrir(Form::Marcador)), false),
                        botao_pequeno(icone("tag", 16.), tr("Referência cruzada"), Some(Msg::FormAbrir(Form::Referencia)), false),
                    ]
                    .spacing(3)
                ]
                .spacing(5)
                .into(),
                tr("Links e comentários")
            ),
            grupo_faixa(
                column![
                    row![
                        botao_pequeno(icone("header-footer", 16.), tr("Cabeçalho"), Some(Msg::FormAbrir(Form::Cabecalho)), false),
                        botao_pequeno(icone("header-footer", 16.), tr("Rodapé"), Some(Msg::FormAbrir(Form::Rodape)), false),
                    ]
                    .spacing(3),
                    menu("Número de página", &["Embaixo, centro", "Embaixo, direita", "Embaixo, esquerda", "Em cima, centro", "Em cima, direita", "Em cima, esquerda", "Página X de Y", "Remover número"],
                        |i| match i {
                            6 => Comando::NumeroPagina(tr("Página {pagina} de {total}"), "base_centro"),
                            7 => Comando::NumeroRemover,
                            _ => Comando::NumeroPagina("{pagina}", ["base_centro", "base_direita", "base_esquerda", "topo_centro", "topo_direita", "topo_esquerda"][i]),
                        }, 150.0),
                ]
                .spacing(4)
                .into(),
                tr("Cabeçalho e rodapé")
            ),
            grupo_faixa(
                column![
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
                ]
                .spacing(5)
                .into(),
                tr("Editar tabela")
            ),
            grupo_faixa(
                column![
                    {
                        // Ligado aparece como botão ativo, sem caractere fazendo papel de ícone.
                        let b = button(text(tr("Cabeçalho")).size(12)).padding([4, 8])
                            .style(estilo_botao_pequeno(app.sessao.tabela_tem_cabecalho()));
                        if em_tabela { b.on_press(Msg::Comando(Comando::TabelaCabecalho)) } else { b }
                    },
                    tabela_botao("Excluir tabela", Comando::TabelaExcluir, em_tabela),
                ]
                .spacing(5)
                .into(),
                tr("Tabela")
            )
        ]
        .into(),
        Aba::Desenhar => row![
            grupo_faixa(
                row![
                    em_breve("pointer", "Selecionar"),
                    em_breve("pencil", "Caneta"),
                    em_breve("highlight", "Marca-texto"),
                    em_breve("eraser", "Borracha"),
                    em_breve("lasso", "Laço"),
                ]
                .into(),
                tr("Ferramentas de desenho")
            ),
            grupo_faixa(
                row![em_breve("hand-stop", "Desenhar com o toque")].into(),
                tr("Toque")
            )
        ]
        .into(),
        Aba::Design => row![
            grupo_faixa(
                column![
                    botao_pequeno(icone("bucket", 16.), tr("Cor da página"), Some(Msg::AbrirCor(AlvoCor::Pagina)), false),
                    botao_pequeno(icone("square", 16.), tr("Borda da página"), Some(Msg::FormAbrir(Form::BordaPagina)), false),
                    botao_pequeno(icone("text-box", 16.), tr("Marca d'água"), Some(Msg::FormAbrir(Form::MarcaDagua)), false),
                ]
                .spacing(3)
                .into(),
                tr("Plano de fundo da página")
            )
        ]
        .into(),
        Aba::Layout => row![
            grupo_faixa(
                column![
                    menu("Margens", &["Normal (2,54 cm)", "Estreita (1,27 cm)", "Moderada", "Larga", "ABNT (3 e 2 cm)", "Personalizadas…"],
                        |i| match i {
                            0 => Comando::Margens([2.54; 4]),
                            1 => Comando::Margens([1.27; 4]),
                            2 => Comando::Margens([1.91, 1.91, 2.54, 2.54]),
                            3 => Comando::Margens([5.08, 5.08, 2.54, 2.54]),
                            4 => Comando::Margens([3.0, 2.0, 3.0, 2.0]),
                            _ => Comando::Formulario(Form::Margens),
                        }, 130.0),
                    botao_pequeno(icone("square", 16.), tr("Papel personalizado"), Some(Msg::FormAbrir(Form::Papel)), false),
                    menu("Texto na página", &["No topo", "No centro", "Na base"],
                        |i| Comando::Vertical(["topo", "centro", "base"][i]), 130.0),
                ]
                .spacing(4)
                .into(),
                tr("Configurar página")
            ),
            grupo_faixa(
                row![
                    grande(
                        "file-plus",
                        "a4",
                        Comando::Papel("a4"),
                        app.sessao.estado().papel == "a4"
                    ),
                    grande(
                        "file-plus",
                        "carta",
                        Comando::Papel("carta"),
                        app.sessao.estado().papel == "carta"
                    )
                ]
                .into(),
                tr("papel")
            ),
            grupo_faixa(
                row![
                    grande(
                        "file-plus",
                        "retrato",
                        Comando::Orientacao(false),
                        !app.sessao.estado().paisagem
                    ),
                    grande(
                        "file-plus",
                        "paisagem",
                        Comando::Orientacao(true),
                        app.sessao.estado().paisagem
                    )
                ]
                .into(),
                tr("orientacao")
            ),
            grupo_faixa(
                row![botao_grande(icone("page-break", 24.), tr("Quebras"),
                    Some(Msg::MenuFaixa((app.menu_faixa != Some(MenuFaixa::Quebras)).then_some(MenuFaixa::Quebras))),
                    app.menu_faixa == Some(MenuFaixa::Quebras))].into(),
                tr("Quebras")
            ),
            grupo_faixa(
                column![
                    row![
                        so_icone("indent-decrease", "Diminuir recuo", Comando::Recuar(-1), false),
                        so_icone("indent-increase", "Aumentar recuo", Comando::Recuar(1), false),
                        botao_de_menu(app, "line-height", "Espaçamento de linha e parágrafo", MenuFaixa::Entrelinha),
                    ]
                    .spacing(2)
                    .align_y(Alignment::Center),
                ]
                .spacing(6)
                .into(),
                tr("paragrafo")
            ),
            grupo_faixa(
                column![
                    menu("Números de linha", &["Nenhum", "Contínuos", "Reiniciar a cada página"],
                        |i| Comando::NumerarLinhas(["nenhuma", "continua", "por_pagina"][i]), 150.0),
                    botao_pequeno(icone("header-footer", 16.), tr("Primeira página diferente"),
                        Some(Msg::Comando(Comando::PrimeiraDiferente)), app.sessao.documento().extras.primeira_diferente),
                ]
                .spacing(4)
                .into(),
                tr("Linhas e capa")
            )
        ]
        .into(),
        Aba::Referencias => row![
            grupo_faixa(
                row![
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
                ]
                .spacing(5)
                .into(),
                tr("Sumário")
            ),
            grupo_faixa(
                column![
                    botao_pequeno(icone("text-box", 16.), tr("Inserir nota de rodapé"), Some(Msg::FormAbrir(Form::NotaRodape)), false),
                    botao_pequeno(icone("text-box", 16.), tr("Inserir nota de fim"), Some(Msg::FormAbrir(Form::NotaFim)), false),
                ]
                .spacing(3)
                .into(),
                tr("Notas de rodapé")
            ),
            grupo_faixa(
                column![
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
                ]
                .spacing(4)
                .into(),
                tr("Citações e bibliografia")
            ),
            grupo_faixa(
                column![
                    menu("Inserir legenda", &["Figura", "Tabela", "Equação"],
                        |i| Comando::Legenda(["figura", "tabela", "equacao"][i]), 150.0),
                    menu("Índice de ilustrações", &["Lista de figuras", "Lista de tabelas"],
                        |i| Comando::ListaDe(i as u8 + 1), 150.0),
                    botao_pequeno(icone("tag", 16.), tr("Referência cruzada"), Some(Msg::FormAbrir(Form::Referencia)), false),
                ]
                .spacing(4)
                .into(),
                tr("Legendas")
            ),
            grupo_faixa(
                column![
                    botao_pequeno(icone("list-details", 16.), tr("Marcar entrada"), Some(Msg::FormAbrir(Form::IndiceEntrada)), false),
                    botao_pequeno(icone("list-details", 16.), tr("Inserir índice"), Some(Msg::Comando(Comando::IndiceInserir)), false),
                ]
                .spacing(3)
                .into(),
                tr("Índice")
            )
        ]
        .into(),
        Aba::Correspondencias => row![
            grupo_faixa(
                row![em_breve("text-box", "Envelopes"), em_breve("tag", "Etiquetas")].into(),
                tr("Criar")
            ),
            grupo_faixa(
                row![em_breve("forms", "Iniciar mala direta"), em_breve("user", "Destinatários")].into(),
                tr("Mala direta")
            )
        ]
        .into(),
        Aba::Revisao => row![
            grupo_faixa(
                row![grande("search", "localizar", Comando::Buscar, false)].into(),
                tr("busca")
            ),
            grupo_faixa(
                row![
                    grande(
                        "file-plus",
                        "editar",
                        Comando::Modo("editando"),
                        app.sessao.modo_edicao() == "editando"
                    ),
                    grande(
                        "file-plus",
                        "sugerir",
                        Comando::Modo("sugerindo"),
                        app.sessao.modo_edicao() == "sugerindo"
                    ),
                    grande(
                        "file-plus",
                        "leitura",
                        Comando::Modo("leitura"),
                        app.sessao.modo_edicao() == "leitura"
                    )
                ]
                .into(),
                tr("modo")
            ),
            grupo_faixa(
                row![
                    botao_grande(icone("arrow-forward-up", 24.), tr("Aceitar"), Some(Msg::Comando(Comando::Revisoes(true, false))), false),
                    botao_grande(icone("arrow-back-up", 24.), tr("Rejeitar"), Some(Msg::Comando(Comando::Revisoes(false, false))), false),
                    column![
                        botao_pequeno(icone("checks", 16.), tr("Aceitar todas"), Some(Msg::Comando(Comando::Revisoes(true, true))), false),
                        botao_pequeno(icone("x", 16.), tr("Rejeitar todas"), Some(Msg::Comando(Comando::Revisoes(false, true))), false),
                        botao_pequeno(icone("columns", 16.), tr("Comparar com arquivo"), Some(Msg::Comando(Comando::Comparar)), false),
                    ]
                    .spacing(3)
                ]
                .into(),
                tr("alteracoes")
            ),
            grupo_faixa(
                row![
                    grande("comentario", "novo_comentario", Comando::Formulario(Form::Comentario), false),
                    grande("layout-sidebar", "painel_revisao", Comando::PainelRevisao, app.painel_revisao()),
                ]
                .into(),
                tr("Comentários")
            ),
            grupo_faixa(
                row![
                    grande("lock", "proteger", Comando::Proteger, app.sessao.documento().extras.protegido),
                ]
                .into(),
                tr("Proteger")
            )
        ]
        .into(),
        Aba::Exibir => row![
            grupo_faixa(
                row![
                    grande(
                        "layout-sidebar",
                        "estrutura",
                        Comando::Estrutura,
                        app.config.estrutura
                    ),
                    grande(
                        "list-details",
                        "marcas",
                        Comando::Marcas,
                        app.sessao.estado().marcas
                    ),
                    grande(
                        "code",
                        "codigo_latex",
                        Comando::Codigo(Fonte::Latex),
                        app.fonte_do_codigo() == Some(Fonte::Latex)
                    ),
                    grande(
                        "file-code",
                        "codigo_markdown",
                        Comando::Codigo(Fonte::Markdown),
                        app.fonte_do_codigo() == Some(Fonte::Markdown)
                    )
                ]
                .into(),
                tr("paineis")
            ),
            grupo_faixa(
                row![
                    grande("zoom-out", "reduzir", Comando::Zoom(-0.1), false),
                    grande("zoom-in", "ampliar", Comando::Zoom(0.1), false)
                ]
                .into(),
                tr("zoom")
            ),
            grupo_faixa(
                row![grande(
                    "settings",
                    "configuracoes",
                    Comando::Configuracoes,
                    false
                )]
                .into(),
                tr("preferencias")
            )
        ]
        .into(),
    };
    column![
        abas,
        // A barra de rolagem ocupa a própria faixa de 6 px: não cobre as legendas dos grupos.
        scrollable(iced::widget::container(conteudo).height(106))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(6).scroller_width(6).spacing(2)
            ))
    ]
    .spacing(0)
    .width(Length::Fill)
    .align_x(Alignment::Start)
    .into()
}
