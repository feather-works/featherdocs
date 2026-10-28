//! Menus clássicos (ARQUIVO, EDITAR, EXIBIR…) no lugar das abas quando a faixa está recolhida
//! (Ctrl+F1), como no FeatherSheet (`barra_menu_classico`/`painel_menu_classico`). Cada item é uma
//! mensagem que a faixa já manda: nada de item morto. Com um menu aberto, passar o mouse em outro
//! título troca de menu, como no Excel 2003.
use crate::{
    app::{App, Comando, Msg},
    cor_ui::AlvoCor,
    dialogos::Tipo as Form,
    faixa::{self, MenuFaixa},
    localidade::tr,
};
use crate::compat::button;
use feather_ui::{icones::icone, tema::*};
use featherdocs_io::dividido::Fonte;
use pelicano::{column, container, mouse_area, row, scrollable, text, Align, Element, Length, Space};

/// Largura do botão Menu (a lista de cada menu se alinha por ela) e de cada título (cabe "REFERÊNCIAS").
const LARGURA_BOTAO_MENU: f32 = 84.;
const LARGURA_TITULO: f32 = 104.;
/// A altura da barra dos menus: a lista desce dali.
pub const ALTURA_BARRA: f32 = 40.;

pub struct Item {
    pub icone: &'static str,
    pub rotulo: &'static str,
    pub atalho: &'static str,
    /// `None` é a linha separadora.
    pub acao: Option<Acao>,
    pub disponivel: bool,
    pub marcado: bool,
    /// Item de cor pronta: o quadradinho com a cor no lugar do ícone.
    pub cor: Option<&'static str>,
}
pub enum Acao {
    Msg(Msg),
    Sub(Vec<Item>),
}
impl Item {
    fn marcado(mut self, sim: bool) -> Self {
        self.marcado = sim;
        self
    }
    fn se(mut self, disponivel: bool) -> Self {
        self.disponivel = disponivel;
        self
    }
}
fn msg(icone: &'static str, rotulo: &'static str, atalho: &'static str, m: Msg) -> Item {
    Item { icone, rotulo, atalho, acao: Some(Acao::Msg(m)), disponivel: true, marcado: false, cor: None }
}
fn cmd(icone: &'static str, rotulo: &'static str, atalho: &'static str, c: Comando) -> Item {
    msg(icone, rotulo, atalho, Msg::Comando(c))
}
fn sub(icone: &'static str, rotulo: &'static str, filhos: Vec<Item>) -> Item {
    Item { icone, rotulo, atalho: "", acao: Some(Acao::Sub(filhos)), disponivel: true, marcado: false, cor: None }
}
const fn sep() -> Item {
    Item { icone: "", rotulo: "", atalho: "", acao: None, disponivel: false, marcado: false, cor: None }
}

/// As cores prontas de `alvo` (o seletor inteiro fica na faixa), marcando a atual.
fn cores(app: &App, alvo: AlvoCor) -> Vec<Item> {
    let atual = crate::cor_ui::cor_de(app, alvo);
    crate::cor_ui::prontas(alvo).iter()
        .map(|&(nome, c)| Item { cor: Some(c), ..msg("", nome, "", Msg::CorEscolhida(alvo, c.into())).marcado(atual.eq_ignore_ascii_case(c)) })
        .collect()
}

/// O catálogo: o título de cada menu e os itens, com o que vale agora (marcado, disponível).
pub fn menus(app: &App) -> Vec<(&'static str, Vec<Item>)> {
    use featherdocs_core::modelo::{CODIGO, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
    let f = &app.visao.formato;
    let doc = app.sessao.documento();
    let em_tabela = f.tabela.is_some();
    let pagina = &doc.pagina;
    let paisagem = pagina.largura > pagina.altura;
    let tabelas = faixa::TABELAS.iter().map(|&(nome, l, c)| cmd("table", nome, "", Comando::TabelaTam(l, c)))
        .chain([sep(), cmd("table", "Personalizada…", "", Comando::Formulario(Form::Tabela))]).collect();
    let graficos = faixa::GRAFICOS.iter().map(|&(t, nome, ic)| cmd(ic, nome, "", Comando::Grafico(t)).se(em_tabela))
        .chain([sep(), cmd("refresh", "Atualizar gráfico selecionado", "", Comando::AtualizarGrafico).se(faixa::em_grafico(app))]).collect();
    let numeros = faixa::NUMERO_PAGINA.iter().enumerate().map(|(i, &nome)| cmd("", nome, "", faixa::numero_pagina(i))).collect();
    let alinhar = [("align-left", "Esquerda", "esquerda", "Ctrl+L"), ("align-center", "Centro", "centro", "Ctrl+E"),
        ("align-right", "Direita", "direita", "Ctrl+R"), ("align-justified", "Justificado", "justificado", "Ctrl+J")]
        .into_iter().map(|(ic, nome, id, at)| cmd(ic, nome, at, Comando::Alinhar(id)).marcado(f.alinhamento == id)).collect();
    let entrelinha = faixa::ENTRELINHAS.iter().map(|&(nome, v)| cmd("", nome, "", Comando::Entrelinha(v)).marcado((f.entrelinha - v).abs() < 0.01)).collect();
    let marcadores = faixa::MARCADORES.iter().map(|&(nome, id)| cmd("list", nome, "", Comando::Lista(id)).marcado(f.estilo == "item" && f.lista == id)).collect();
    let numeracao = faixa::NUMERACOES.iter().map(|&(nome, id)| cmd("list-numbers", nome, "", Comando::Lista(id)).marcado(f.estilo == "item_ordenado" && f.lista == id))
        .chain([sep(), cmd("refresh", "Reiniciar numeração em 1", "", Comando::ListaReiniciar)]).collect();
    let bordas = faixa::BORDAS.iter().map(|&(nome, bits)| cmd("square", nome, "", Comando::Borda(bits)).marcado(f.borda == bits)).collect();
    let maiusculas = faixa::MAIUSCULAS.iter().map(|&(nome, id)| cmd("text-size", nome, "", Comando::Maiusculas(id))).collect();
    let estilos = faixa::DE_FABRICA.iter().map(|&(id, nome, ..)| cmd("", nome, "", Comando::Estilo(id)).marcado(f.estilo == id))
        .chain([sep(), cmd("plus", "Novo estilo…", "", Comando::Formulario(Form::NovoEstilo)),
            cmd("wand", "Aplicar estilos…", "", Comando::Formulario(Form::AplicarEstilo))]).collect();
    let margens = faixa::MARGENS.iter().map(|&(nome, _, cm)| cmd("borda_externa_espessa", nome, "", Comando::Margens(cm)).marcado(faixa::margens_iguais(pagina, cm)))
        .chain([sep(), cmd("settings", "Margens personalizadas…", "", Comando::Formulario(Form::Margens))]).collect();
    let papel = app.sessao.estado().papel;
    let tamanhos = faixa::PAPEIS.iter().map(|&(id, nome, _)| cmd("file", nome, "", Comando::Papel(id)).marcado(papel == id))
        .chain([sep(), cmd("settings", "Mais tamanhos de papel…", "", Comando::Formulario(Form::Papel))]).collect();
    let linhas = faixa::NUMEROS_DE_LINHA.iter().map(|&(nome, id, v)| cmd("", nome, "", Comando::NumerarLinhas(id)).marcado(doc.extras.numerar_linhas == v)).collect();
    let vertical = faixa::VERTICAIS.iter().map(|&(nome, id, ic, v)| cmd(ic, nome, "", Comando::Vertical(id)).marcado(doc.extras.vertical == v)).collect();
    let quebras = faixa::QUEBRAS.iter().map(|&(ic, nome, _, c)| cmd(ic, nome, if nome == "Página" { "Ctrl+Enter" } else { "" }, faixa::comando_quebra(c))).collect();
    let sumario = (0..3u8).map(|i| cmd("list-details", ["Até o nível 2", "Até o nível 3", "Até o nível 4"][usize::from(i)], "", Comando::SumarioNiveis(i + 2)))
        .chain([sep(), cmd("trash", "Remover sumários", "", Comando::SumarioRemover)]).collect();
    let modos = [("Editando", "editando"), ("Sugerindo", "sugerindo"), ("Leitura", "leitura")].into_iter()
        .map(|(nome, id)| cmd("edit", nome, "", Comando::Modo(id)).marcado(app.sessao.modo_edicao() == id)).collect();
    vec![
        ("Arquivo", vec![
            cmd("file-plus", "Novo", "Ctrl+N", Comando::Novo),
            cmd("folder-open", "Abrir…", "Ctrl+O", Comando::Abrir),
            sep(),
            cmd("device-floppy", "Salvar", "Ctrl+S", Comando::Salvar),
            cmd("device-floppy", "Salvar como…", "Ctrl+Shift+S", Comando::SalvarComo),
            cmd("file-export", "Exportar…", "", Comando::Exportar),
            cmd("printer", "Exportar PDF…", "Ctrl+P", Comando::ExportarPdf),
            sep(),
            cmd("settings", "Configurações", "Ctrl+,", Comando::Configuracoes),
        ]),
        ("Editar", vec![
            cmd("arrow-back-up", "Desfazer", "Ctrl+Z", Comando::Desfazer),
            cmd("arrow-forward-up", "Refazer", "Ctrl+Y", Comando::Refazer),
            sep(),
            cmd("cut", "Recortar", "Ctrl+X", Comando::Recortar),
            cmd("copy", "Copiar", "Ctrl+C", Comando::Copiar),
            cmd("clipboard", "Colar", "Ctrl+V", Comando::Colar),
            cmd("clipboard", "Colar sem formatação", "Ctrl+Shift+V", Comando::ColarSemFormatacao),
            cmd("brush", "Pincel de formatação", "", Comando::Pincel).marcado(f.pincel),
            sep(),
            cmd("search", "Localizar e substituir…", "Ctrl+F", Comando::Buscar),
        ]),
        ("Exibir", vec![
            cmd("layout-sidebar", "Estrutura", "", Comando::Estrutura).marcado(app.config.estrutura),
            cmd("list-details", "Marcas de formatação", "", Comando::Marcas).marcado(app.sessao.estado().marcas),
            cmd("code", "Código LaTeX", "", Comando::Codigo(Fonte::Latex)).marcado(app.fonte_do_codigo() == Some(Fonte::Latex)),
            cmd("file-code", "Código Markdown", "", Comando::Codigo(Fonte::Markdown)).marcado(app.fonte_do_codigo() == Some(Fonte::Markdown)),
            cmd("layout-sidebar", "Painel de revisão", "", Comando::PainelRevisao).marcado(app.painel_revisao()),
            sep(),
            cmd("zoom-in", "Ampliar", "", Comando::Zoom(0.1)),
            cmd("zoom-out", "Reduzir", "", Comando::Zoom(-0.1)),
            sep(),
            msg("recolher_faixa", "Expandir faixa", "Ctrl+F1", Msg::AlternarFaixa),
        ]),
        ("Inserir", vec![
            sub("table", "Tabela", tabelas),
            cmd("photo", "Imagem…", "", Comando::Imagem),
            sub("chart-bar", "Gráfico", graficos),
            sep(),
            cmd("page-break", "Quebra de página", "Ctrl+Enter", Comando::Quebra),
            cmd("equacao", "Equação", "", Comando::Estilo("equacao")),
            cmd("minus", "Linha horizontal", "", Comando::Estilo("regua")),
            sep(),
            cmd("link", "Link…", "Ctrl+K", Comando::Formulario(Form::Link)),
            msg("comentario", "Comentário…", "", Msg::FormAbrir(Form::Comentario)),
            msg("tag", "Marcador…", "", Msg::FormAbrir(Form::Marcador)),
            msg("tag", "Referência cruzada…", "", Msg::FormAbrir(Form::Referencia)),
            sep(),
            msg("header-footer", "Cabeçalho…", "", Msg::FormAbrir(Form::Cabecalho)),
            msg("header-footer", "Rodapé…", "", Msg::FormAbrir(Form::Rodape)),
            sub("file", "Número de página", numeros),
        ]),
        ("Formatar", vec![
            cmd("bold", "Negrito", "Ctrl+B", Comando::Formato(NEGRITO)).marcado(f.negrito),
            cmd("italic", "Itálico", "Ctrl+I", Comando::Formato(ITALICO)).marcado(f.italico),
            cmd("underline", "Sublinhado", "Ctrl+U", Comando::Formato(SUBLINHADO)).marcado(f.sublinhado),
            cmd("strikethrough", "Tachado", "Ctrl+Shift+X", Comando::Formato(TACHADO)).marcado(f.tachado),
            cmd("superscript", "Sobrescrito", "Ctrl+Shift+=", Comando::Posicao(1)).marcado(f.sobrescrito),
            cmd("subscript", "Subscrito", "Ctrl+=", Comando::Posicao(-1)).marcado(f.subscrito),
            cmd("code", "Código", "Ctrl+Shift+E", Comando::Formato(CODIGO)).marcado(f.codigo),
            sub("text-size", "Maiúsculas e minúsculas", maiusculas),
            sub("font-color", "Cor do texto", cores(app, AlvoCor::Texto)),
            sub("highlight", "Marca-texto", cores(app, AlvoCor::Realce)),
            cmd("clear", "Limpar formatação", "Ctrl+Espaço", Comando::LimparFormatacao),
            sep(),
            sub("align-left", "Alinhamento", alinhar),
            sub("line-height", "Entrelinha", entrelinha),
            sub("list", "Marcadores", marcadores),
            sub("list-numbers", "Numeração", numeracao),
            cmd("indent-decrease", "Diminuir recuo", "Shift+Tab", Comando::Recuar(-1)),
            cmd("indent-increase", "Aumentar recuo", "Tab", Comando::Recuar(1)),
            sub("square", "Bordas", bordas),
            sub("bucket", "Sombreamento", cores(app, AlvoCor::Fundo)),
            sep(),
            sub("palette", "Estilos", estilos),
        ]),
        ("Layout", vec![
            sub("borda_externa_espessa", "Margens", margens),
            sub("orientacao_pagina", "Orientação", vec![
                cmd("file", "Retrato", "", Comando::Orientacao(false)).marcado(!paisagem),
                cmd("orientacao_pagina", "Paisagem", "", Comando::Orientacao(true)).marcado(paisagem),
            ]),
            sub("arrows-maximize", "Tamanho", tamanhos),
            sub("quebra_pagina", "Quebras", quebras),
            sub("list-numbers", "Números de linha", linhas),
            sub("layout-align-middle", "Alinhamento vertical", vertical),
            sep(),
            cmd("header-footer", "Primeira página diferente", "", Comando::PrimeiraDiferente).marcado(doc.extras.primeira_diferente),
            sub("bucket", "Cor da página", cores(app, AlvoCor::Pagina)),
            msg("square", "Borda da página…", "", Msg::FormAbrir(Form::BordaPagina)),
            msg("text-box", "Marca d'água…", "", Msg::FormAbrir(Form::MarcaDagua)),
        ]),
        ("Referências", vec![
            cmd("list-details", "Inserir sumário", "", Comando::Sumario),
            sub("list-details", "Sumário", sumario),
            cmd("refresh", "Atualizar sumário e referências", "", Comando::ReferenciasAtualizar),
            sep(),
            msg("text-box", "Inserir nota de rodapé…", "", Msg::FormAbrir(Form::NotaRodape)),
            msg("text-box", "Inserir nota de fim…", "", Msg::FormAbrir(Form::NotaFim)),
            sep(),
            msg("plus", "Nova fonte…", "", Msg::FormAbrir(Form::NovaFonte)),
            cmd("file-import", "Importar .bib…", "", Comando::ImportarBib),
            msg("message-circle", "Inserir citação…", "", Msg::FormAbrir(Form::Citacao)),
            sub("list-details", "Bibliografia", vec![
                cmd("list-details", "Só as citadas", "", Comando::Bibliografia(false)),
                cmd("list-details", "Todas as fontes", "", Comando::Bibliografia(true)),
            ]),
            sep(),
            sub("tag", "Inserir legenda", ["Figura", "Tabela", "Equação"].into_iter().zip(["figura", "tabela", "equacao"])
                .map(|(nome, id)| cmd("tag", nome, "", Comando::Legenda(id))).collect()),
            sub("list-details", "Índice de ilustrações", vec![
                cmd("photo", "Lista de figuras", "", Comando::ListaDe(1)),
                cmd("table", "Lista de tabelas", "", Comando::ListaDe(2)),
            ]),
            msg("list-details", "Marcar entrada do índice…", "", Msg::FormAbrir(Form::IndiceEntrada)),
            cmd("list-details", "Inserir índice", "", Comando::IndiceInserir),
        ]),
        ("Revisão", vec![
            sub("edit", "Modo", modos),
            sep(),
            cmd("arrow-forward-up", "Aceitar", "", Comando::Revisoes(true, false)),
            cmd("arrow-back-up", "Rejeitar", "", Comando::Revisoes(false, false)),
            cmd("checks", "Aceitar todas", "", Comando::Revisoes(true, true)),
            cmd("x", "Rejeitar todas", "", Comando::Revisoes(false, true)),
            cmd("columns", "Comparar com arquivo…", "", Comando::Comparar),
            sep(),
            cmd("comentario", "Novo comentário…", "", Comando::Formulario(Form::Comentario)),
            cmd("lock", "Proteger documento", "", Comando::Proteger).marcado(doc.extras.protegido),
        ]),
        ("Ajuda", vec![
            cmd("help", "Ajuda", "F1", Comando::Ajuda),
            cmd("device-desktop", "Monitor", "F12", Comando::Monitor),
        ]),
    ]
}

/// O botão "≡ Menu" e os títulos dos menus, no lugar das abas da faixa.
pub fn barra(app: &App) -> Element<'_, Msg> {
    let aberto = match app.menu_faixa {
        Some(MenuFaixa::Classico(m, _)) => Some(usize::from(m)),
        _ => None,
    };
    let botao_menu = button(row![icone("menu-2", 14.), text(tr("Menu")).size(13)].spacing(6).align(Align::Center))
        .width(LARGURA_BOTAO_MENU)
        .padding([7, 12])
        .style(estilo_botao_secundario)
        .on_press(Msg::Comando(Comando::Menu));
    let mut titulos = row![botao_menu].spacing(2).align(Align::Center);
    for (i, (titulo, _)) in menus(app).iter().enumerate() {
        let alvo = (aberto != Some(i)).then_some(MenuFaixa::Classico(i as u8, None));
        let botao = button(text(tr(titulo).to_uppercase()).size(11.5).align_x(Align::Center).width(Length::FILL))
            .height(22.)
            .width(LARGURA_TITULO)
            .padding([0, 5])
            .style(estilo_titulo_menu(aberto == Some(i)))
            .on_press(Msg::MenuFaixa(alvo));
        titulos = titulos.push(if aberto.is_some() {
            Element::from(mouse_area(botao).on_enter(Msg::MenuFaixa(Some(MenuFaixa::Classico(i as u8, None)))))
        } else {
            botao.into()
        });
    }
    container(row![
        scrollable(titulos).direction(pelicano::Direction::Horizontal).width(Length::FILL).height(Length::Shrink),
        faixa::botao_recolher(app),
    ].spacing(6).align(Align::Center))
    .height(ALTURA_BARRA)
    .padding([0, 8])
    .align_y(Align::Center)
    .into()
}

/// Onde a lista do menu `m` começa (a borda esquerda do título dele), presa dentro da janela.
pub fn esquerda(m: u8, largura_janela: f32) -> f32 {
    (8. + LARGURA_BOTAO_MENU + 2. + f32::from(m) * (LARGURA_TITULO + 2.)).min((largura_janela - LARGURA_MENU - 8.).max(0.))
}

/// A lista do menu aberto; num submenu, a linha "‹ Voltar" com o caminho no topo.
pub fn painel(app: &App, m: u8, sub: Option<u8>, altura_disponivel: f32) -> Element<'static, Msg> {
    let mut todos = menus(app);
    let Some((titulo, itens)) = (usize::from(m) < todos.len()).then(|| todos.swap_remove(usize::from(m))) else {
        return Space::new(0, 0).into();
    };
    let mut conteudo = column![].spacing(0).padding([7, 6]);
    let mut itens = itens;
    let mut altura: f32 = 14.;
    if let Some(s) = sub {
        let Some(Item { rotulo, acao: Some(Acao::Sub(filhos)), .. }) = (usize::from(s) < itens.len()).then(|| itens.swap_remove(usize::from(s))) else {
            return Space::new(0, 0).into();
        };
        conteudo = conteudo.push(
            button(row![
                text("‹").size(17),
                text(tr("Voltar")).size(12.5),
                Space::new(Length::FILL, 0.),
                text(format!("{} › {}", tr(titulo), tr(rotulo))).size(10).color(COR_TEXTO_MUTED()),
            ].spacing(7).align(Align::Center))
            .width(Length::FILL)
            .height(32.)
            .padding([5, 8])
            .style(estilo_item_menu(false, true))
            .on_press(Msg::MenuFaixa(Some(MenuFaixa::Classico(m, None)))),
        );
        altura += 32.;
        itens = filhos;
    }
    for (i, item) in itens.into_iter().enumerate() {
        let Some(acao) = item.acao else {
            conteudo = conteudo.push(container(container(Space::new(0., 1.)).width(Length::FILL).style(estilo_linha)).padding([6, 0]));
            altura += 13.;
            continue;
        };
        let marca: Element<'static, Msg> = if item.marcado {
            container(text("✓").size(13).color(COR_AZUL_ACENTO())).width(16.).into()
        } else if let Some(c) = item.cor {
            crate::cor_ui::quadradinho(c)
        } else if item.icone.is_empty() {
            Space::new(16., 16.).into()
        } else {
            icone(item.icone, 16.).into()
        };
        let mut linha = row![marca, text(tr(item.rotulo).to_string()).size(13.5), Space::new(Length::FILL, 0.)]
            .spacing(12)
            .align(Align::Center);
        if !item.atalho.is_empty() {
            linha = linha.push(text(item.atalho).size(12).color(COR_TEXTO_MUTED()));
        }
        let alvo = match acao {
            Acao::Sub(_) => {
                linha = linha.push(text("▸").size(11).color(COR_TEXTO_MUTED()));
                Msg::MenuFaixa(Some(MenuFaixa::Classico(m, Some(i as u8))))
            }
            // O menu fecha antes da ação (a do botão direito também passa por aqui).
            Acao::Msg(msg) => Msg::DoMenu(Box::new(msg)),
        };
        conteudo = conteudo.push(
            button(linha)
                .height(32.)
                .width(Length::FILL)
                .padding([5, 8])
                .style(estilo_item_menu(false, item.disponivel))
                .on_press_maybe(item.disponivel.then_some(alvo)),
        );
        altura += 32.;
    }
    container(scrollable(conteudo).height(altura.min(altura_disponivel.max(150.))))
        .width(LARGURA_MENU)
        .style(estilo_popup_menu)
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Todo item leva a uma mensagem que existe (ou a um submenu com itens); nenhum submenu vazio
    /// nem rótulo repetido no mesmo menu; e escolher cada um não derruba o aplicativo.
    #[test]
    fn todo_item_do_catalogo_leva_a_um_comando() {
        let app = App::default();
        let catalogo = menus(&app);
        assert!(catalogo.len() >= 8);
        let mut folhas = 0;
        for (titulo, itens) in &catalogo {
            assert!(!itens.is_empty(), "{titulo} vazio");
            let mut vistos = std::collections::HashSet::new();
            for item in itens {
                match &item.acao {
                    None => continue,
                    Some(Acao::Sub(filhos)) => {
                        assert!(filhos.iter().any(|f| f.acao.is_some()), "{titulo} › {} sem itens", item.rotulo);
                        assert!(filhos.iter().all(|f| !matches!(f.acao, Some(Acao::Sub(_)))), "um nível de submenu só");
                        folhas += filhos.iter().filter(|f| f.acao.is_some()).count();
                    }
                    Some(Acao::Msg(_)) => folhas += 1,
                }
                assert!(vistos.insert(item.rotulo), "{titulo}: {} repetido", item.rotulo);
            }
        }
        assert!(folhas > 100, "só {folhas} comandos");
    }

    #[test]
    fn abrir_trocar_entrar_no_submenu_e_escolher() {
        let mut app = App::default();
        let principal = crate::compat::window::novo_id();
        app.config.faixa_recolhida = true;
        let _ = app.update(Msg::MenuFaixa(Some(MenuFaixa::Classico(1, None))));
        let _ = app.view(principal);
        // Inserir › Tabela › 2 × 2 insere a tabela e fecha o menu.
        let _ = app.update(Msg::MenuFaixa(Some(MenuFaixa::Classico(3, Some(0)))));
        let _ = app.view(principal);
        let antes = app.sessao.documento().blocos.len();
        let Some(Acao::Sub(filhos)) = menus(&app).swap_remove(3).1.swap_remove(0).acao else { panic!("Tabela é submenu") };
        let Some(Acao::Msg(m)) = filhos.into_iter().next().and_then(|i| i.acao) else { panic!() };
        let _ = app.update(Msg::DoMenu(Box::new(m)));
        assert!(app.menu_faixa.is_none());
        assert!(app.sessao.documento().blocos.len() > antes);
        for m in 0..menus(&app).len() as u8 {
            let _ = painel(&app, m, None, 600.);
        }
    }
}
