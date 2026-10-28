//! Tela inicial e backstage (menu Arquivos), no molde do FeatherSheet (`menu_principal.rs` de lá):
//! marca e Abrir, faixa "Novo" com os modelos (miniatura = a primeira página de verdade de cada
//! um) e a lista de recentes com busca. Serve à janela inteira na abertura sem arquivo e ao
//! painel do menu Arquivos, com o menu lateral.
use crate::app::{App, Comando, Msg};
use crate::localidade::{tr, trf};
use crate::pagina::{Estado as EstadoPagina, Pagina};
use crate::quadro::Quadro;
use featherdocs_core::{geometria::Visao, modelos, sessao::Sessao};
use feather_ui::tema::*;
use pelicano::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, tooltip, Align, Background, Border, Color,
    Element, Font, Length, Point, Program, Rect, Renderer, Shadow, Space, Status, Style, Theme, TooltipPosition,
};
use std::path::{Path, PathBuf};

/// A página do backstage aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaginaMenu {
    #[default]
    Inicio,
    Recentes,
    Exportar,
    Informacoes,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Pagina(PaginaMenu),
    Modelo(&'static str),
    Buscar(String),
    Carrossel(i32),
    VerTodos,
    Esquecer(usize),
}

/// Estado da tela inicial e do backstage.
pub struct Inicio {
    /// Janela inteira com a tela inicial (abertura sem arquivo).
    pub tela_cheia: bool,
    pub pagina: PaginaMenu,
    pub busca: String,
    pub carrossel: usize,
    pub ver_todos: bool,
    /// A primeira página de cada modelo, montada uma vez (é o motor que pagina).
    miniaturas: Vec<(&'static str, Quadro)>,
    visao: Visao,
}

impl Default for Inicio {
    fn default() -> Self {
        // A vista sem cursor nem seleção: a da primeira página do documento vazio, sem o cursor.
        let mut visao = Sessao::default().visao(0, 0);
        visao.cursor = None;
        visao.retangulos.clear();
        let miniaturas = modelos::lista().iter().filter_map(|m| {
            let s = Sessao::com_documento(modelos::criar(m.id)?);
            Some((m.id, s.quadro(0, u64::MAX).and_then(|b| Quadro::ler(&b)).ok()?))
        }).collect();
        Self {
            tela_cheia: false,
            pagina: PaginaMenu::Inicio,
            busca: String::new(),
            carrossel: 0,
            ver_todos: false,
            miniaturas,
            visao,
        }
    }
}

const CARTAO_LARGURA: f32 = 176.0;
const CARTAO_ALTURA: f32 = 252.0;
const CARTAO_ESPACO: f32 = 14.0;
const MINIATURA_ALTURA: f32 = 162.0;
const SETA: f32 = 32.0;
const CONTEUDO_MAX: f32 = 1180.0;
const LATERAL: f32 = 217.0;
const COL_TIPO: f32 = 46.0;
const COL_QUANDO: f32 = 104.0;
const COL_ACOES: f32 = 32.0;

/// Só desenha a página (a do editor também trata clique e teclado, que aqui não servem).
struct Miniatura<'a>(Pagina<'a>);

impl<M> Program<M> for Miniatura<'_> {
    type State = ();
    fn draw(&self, _: &(), renderer: &mut Renderer, bounds: Rect, tema: &Theme, cursor: Option<Point>) {
        <Pagina as Program<Msg>>::draw(&self.0, &EstadoPagina::default(), renderer, bounds, tema, cursor);
    }
}

fn negrito() -> Font {
    Font { weight: 600, ..Font::DEFAULT }
}

/// Corta `s` em `max` caracteres com reticências no fim.
fn cortar_fim(s: &str, max: usize) -> String {
    if s.chars().count() <= max.max(2) {
        return s.to_owned();
    }
    let mut curto: String = s.chars().take(max.max(2) - 1).collect();
    curto.push('…');
    curto
}

/// Corta `s` mantendo o fim (o fim de uma pasta diz mais).
fn cortar_inicio(s: &str, max: usize) -> String {
    let total = s.chars().count();
    if total <= max.max(2) {
        return s.to_owned();
    }
    let resto: String = s.chars().skip(total - (max.max(2) - 1)).collect();
    format!("…{resto}")
}

/// Pasta do arquivo, com a pasta pessoal abreviada para `~`.
fn pasta_legivel(p: &Path) -> String {
    let pasta = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && pasta.starts_with(&home) => format!("~{}", &pasta[home.len()..]),
        _ => pasta,
    }
}

/// Selo do tipo: rótulo e cor (azul Word, cinza texto, cor da suíte no .fdoz).
fn selo_tipo(p: &Path) -> (String, Color) {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_uppercase();
    let cor = match ext.as_str() {
        "DOCX" | "DOC" => Color::rgb8(0x2B, 0x57, 0x9A),
        "TEX" | "ZIP" => Color::rgb8(0x00, 0x80, 0x80),
        "MD" => Color::rgb8(0xC2, 0x6A, 0x00),
        "FDOZ" => COR_AZUL_ACENTO(),
        _ => COR_TEXTO_MUTED(),
    };
    let rotulo = if ext.is_empty() { "—".into() } else { ext.chars().take(4).collect() };
    (rotulo, cor)
}

/// Quanto tempo desde a última gravação do arquivo: "há 5 minutos", "ontem", "28/10/2026"
/// (relativo, como os recentes do FeatherSheet: não depende do fuso).
fn quando(p: &Path) -> String {
    let Some(t) = std::fs::metadata(p).and_then(|m| m.modified()).ok() else { return String::new() };
    let s = std::time::SystemTime::now().duration_since(t).map_or(0, |d| d.as_secs());
    match s {
        0..=59 => tr("agora").into(),
        60..=3599 => trf("há {} min", &[&(s / 60)]),
        3600..=86_399 => trf("há {} h", &[&(s / 3600)]),
        86_400..=172_799 => tr("ontem").into(),
        _ => {
            let dias = t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) / 86_400;
            let (a, m, d) = civil(dias);
            format!("{d:02}/{m:02}/{a}")
        }
    }
}

/// Dias desde 1970 para (ano, mês, dia), o algoritmo de Howard Hinnant.
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn estilo_botao_link(tema: &Theme, estado: Status) -> Style {
    let mut e = estilo_botao_secundario(tema, estado);
    e.border.width = 0.0;
    if !matches!(estado, Status::Hovered) {
        e.background = None;
    }
    e
}

fn botao_link(rotulo: String, icone: &'static str, m: Msg) -> Element<'static, Msg> {
    button(row![text(rotulo).size(12.5).color(COR_AZUL_ACENTO()), feather_ui::icones::icone(icone, 14.0).color(COR_AZUL_ACENTO())]
        .spacing(3).align(Align::Center))
        .padding([3, 6]).style(estilo_botao_link).on_press(m).into()
}

fn seta(nome: &'static str, d: i32, ativa: bool) -> Element<'static, Msg> {
    button(container(feather_ui::icones::icone(nome, 16.0)).center())
        .width(Length::Fixed(SETA)).height(Length::Fixed(SETA)).padding(0)
        .style(move |tema, estado| {
            let mut e = estilo_botao_secundario(tema, estado);
            e.border.radius = [SETA / 2.0; 4];
            if !ativa {
                e.background = None;
                e.border.width = 0.0;
            }
            e
        })
        .on_press_maybe(ativa.then_some(Msg::Inicio(Evento::Carrossel(d))))
        .into()
}

fn divisor<'a>() -> Element<'a, Msg> {
    container(Space::new(0.0, Length::Fixed(1.0))).width(Length::FILL).style(estilo_linha).into()
}

impl App {
    fn novo_de_modelo(&mut self, id: &'static str) -> pelicano::Task<Msg> {
        self.solicitar(crate::app::Destino::Modelo(id))
    }

    pub(crate) fn evento_inicio(&mut self, e: Evento) -> pelicano::Task<Msg> {
        match e {
            Evento::Pagina(p) => self.inicio.pagina = p,
            Evento::Modelo(id) => return self.novo_de_modelo(id),
            Evento::Buscar(t) => self.inicio.busca = t,
            Evento::Carrossel(d) => {
                let total = modelos::lista().len();
                self.inicio.carrossel = (self.inicio.carrossel as i64 + i64::from(d)).clamp(0, total.saturating_sub(1) as i64) as usize;
            }
            Evento::VerTodos => self.inicio.ver_todos = !self.inicio.ver_todos,
            Evento::Esquecer(i) => {
                if i < self.config.recentes.len() {
                    self.config.recentes.remove(i);
                    self.config.gravar();
                }
            }
        }
        pelicano::Task::none()
    }

    /// Largura útil do conteúdo: a janela inteira (tela inicial) ou o painel do backstage.
    fn largura_inicio(&self) -> f32 {
        let janela = if self.tamanho_principal.width > 0.0 { self.tamanho_principal.width } else { 1200.0 };
        if self.inicio.tela_cheia {
            return (janela - 88.0).clamp(CARTAO_LARGURA * 2.0, CONTEUDO_MAX);
        }
        let painel = (janela - 20.0).min(960.0).max(310.0);
        (painel - LATERAL - 46.0).max(CARTAO_LARGURA)
    }

    fn cartao_modelo(&self, m: &'static modelos::ModeloInfo) -> Element<'_, Msg> {
        let interno = CARTAO_LARGURA - 16.0;
        let desenho: Element<'_, Msg> = match self.inicio.miniaturas.iter().find(|(id, _)| *id == m.id) {
            Some((_, q)) => {
                let escala = (MINIATURA_ALTURA / q.altura).min(interno / q.largura);
                let pagina = Pagina { quadro: q, visao: &self.inicio.visao, numero: 0, escala, ativa: false, imagens: &self.imagens, arrastando: false };
                container(crate::compat::canvas(Miniatura(pagina)).width(q.largura * escala).height(q.altura * escala))
                    .style(|_| Style {
                        border: Border { color: COR_BORDA(), width: 1.0, radius: [2.0; 4] },
                        shadow: Some(Shadow { color: Color::rgb8(0, 0, 0).alpha(0.18), offset: Point::new(0.0, 1.0), blur: 4.0 }),
                        ..Default::default()
                    })
                    .into()
            }
            None => Space::new(0.0, 0.0).into(),
        };
        let mut nome = text(cortar_fim(tr(m.nome), 30)).size(13).font(negrito());
        if m.id == "em-branco" {
            nome = nome.color(COR_AZUL_ACENTO());
        }
        button(column![
            container(desenho).width(Length::Fixed(interno)).height(Length::Fixed(MINIATURA_ALTURA)).center(),
            container(nome).width(Length::Fixed(interno)).height(Length::Fixed(20.0)),
            text(cortar_fim(tr(m.descricao), 64)).size(11).color(COR_TEXTO_MUTED()).width(Length::Fixed(interno)),
        ].spacing(6))
            .padding(8)
            .width(Length::Fixed(CARTAO_LARGURA))
            .height(Length::Fixed(CARTAO_ALTURA))
            .style(|_, estado| {
                let sobre = matches!(estado, Status::Hovered);
                Style {
                    background: Some(Background::Color(if sobre { COR_HOVER() } else { COR_PAINEL() })),
                    text_color: Some(COR_TEXTO()),
                    border: Border { color: if sobre { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() }, width: 1.0, radius: [8.0; 4] },
                    shadow: Some(Shadow {
                        color: Color::rgb8(0, 0, 0).alpha(if sobre { 0.14 } else { 0.06 }),
                        offset: Point::new(0.0, if sobre { 2.0 } else { 1.0 }),
                        blur: if sobre { 8.0 } else { 3.0 },
                    }),
                    ..Default::default()
                }
            })
            .on_press(Msg::Inicio(Evento::Modelo(m.id)))
            .into()
    }

    /// Faixa "Novo": os modelos num carrossel de cartões iguais, ou todos em grade.
    fn galeria(&self, largura: f32) -> Element<'_, Msg> {
        let lista = modelos::lista();
        let total = lista.len();
        let passo = CARTAO_LARGURA + CARTAO_ESPACO;
        let cabem = ((largura + CARTAO_ESPACO) / passo).floor().max(1.0) as usize;
        let visiveis = if cabem >= total {
            total
        } else {
            (((largura - 2.0 * (SETA + CARTAO_ESPACO) + CARTAO_ESPACO) / passo).floor().max(1.0) as usize).min(total)
        };
        let inicio = self.inicio.carrossel.min(total - visiveis);
        let rodando = !self.inicio.ver_todos && visiveis < total;
        let mut cabecalho = row![text(tr("Novo")).size(16).font(negrito()), Space::new(Length::FILL, 0.0)].spacing(10).align(Align::Center);
        if rodando {
            cabecalho = cabecalho.push(text(format!("{}–{} {} {}", inicio + 1, inicio + visiveis, tr("de"), total)).size(11.5).color(COR_TEXTO_MUTED()));
        }
        if visiveis < total || self.inicio.ver_todos {
            cabecalho = cabecalho.push(botao_link(
                tr(if self.inicio.ver_todos { "Mostrar menos" } else { "Ver todos os modelos" }).into(),
                if self.inicio.ver_todos { "chevron-up" } else { "chevron-right" },
                Msg::Inicio(Evento::VerTodos),
            ));
        }
        let cartoes: Vec<Element<'_, Msg>> = lista.iter().map(|m| self.cartao_modelo(m)).collect();
        let corpo: Element<'_, Msg> = if !rodando {
            let mut grade = column![].spacing(CARTAO_ESPACO);
            let mut resto = cartoes.into_iter().peekable();
            while resto.peek().is_some() {
                grade = grade.push(resto.by_ref().take(cabem.max(1)).fold(row![].spacing(CARTAO_ESPACO), |l, c| l.push(c)));
            }
            grade.into()
        } else {
            let faixa = cartoes.into_iter().skip(inicio).take(visiveis)
                .fold(row![seta("chevron-left", -1, inicio > 0)].spacing(CARTAO_ESPACO).align(Align::Center), |l, c| l.push(c));
            faixa.push(seta("chevron-right", 1, inicio + visiveis < total)).into()
        };
        column![cabecalho, corpo].spacing(12).into()
    }

    /// Uma linha dos recentes: selo do tipo, nome, pasta, quando e tirar da lista.
    fn linha_recente(&self, i: usize, p: &Path, largura: f32) -> Element<'_, Msg> {
        let existe = p.is_file();
        let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let livre = (largura - COL_TIPO - COL_QUANDO - COL_ACOES - 40.0).max(120.0);
        let (max_nome, max_pasta) = ((livre * 5.0 / 9.0 / 7.2) as usize, (livre * 4.0 / 9.0 / 6.6) as usize);
        let (rotulo, cor) = selo_tipo(p);
        let selo = container(text(rotulo).size(9.5).font(negrito()).color(Color::WHITE)).padding([2, 5]).style(move |_| Style {
            background: Some(Background::Color(if existe { cor } else { COR_TEXTO_MUTED() })),
            border: Border { radius: [4.0; 4], ..Default::default() },
            ..Default::default()
        });
        let mut nome_txt = text(cortar_fim(nome, max_nome)).size(13);
        if !existe {
            nome_txt = nome_txt.color(COR_TEXTO_MUTED());
        }
        let pasta = if existe { cortar_inicio(&pasta_legivel(p), max_pasta) } else { tr("Arquivo não encontrado").to_owned() };
        let linha = button(row![
            container(selo).width(Length::Fixed(COL_TIPO)),
            container(nome_txt).width(Length::Fill(5)),
            container(text(pasta).size(12).color(COR_TEXTO_MUTED())).width(Length::Fill(4)),
            container(text(quando(p)).size(12).color(COR_TEXTO_MUTED())).width(Length::Fixed(COL_QUANDO)),
        ].spacing(10).align(Align::Center))
            .width(Length::FILL)
            .padding([8, 10])
            .style(|_, estado| Style {
                background: matches!(estado, Status::Hovered).then(|| Background::Color(COR_HOVER())),
                text_color: Some(COR_TEXTO()),
                border: Border { radius: [6.0; 4], ..Default::default() },
                ..Default::default()
            })
            .on_press_maybe(existe.then(|| Msg::AbrirRecente(p.to_path_buf())));
        let tirar = button(container(feather_ui::icones::icone("x", 15.0)).center())
            .width(Length::Fixed(26.0)).height(Length::Fixed(26.0)).padding(0)
            .style(|tema, estado| {
                let mut e = estilo_botao_pequeno(false)(tema, estado);
                e.border.radius = [13.0; 4];
                if !matches!(estado, Status::Hovered) {
                    e.background = None;
                    e.border.width = 0.0;
                }
                e
            })
            .on_press(Msg::Inicio(Evento::Esquecer(i)));
        row![
            linha,
            container(tooltip(tirar, tr("Tirar da lista"), TooltipPosition::Top).style(estilo_dica)).width(Length::Fixed(COL_ACOES)),
        ].spacing(4).align(Align::Center).into()
    }

    /// Recentes em lista com busca; no backstage, seis e "Ver todos".
    fn recentes(&self, largura: f32, todos: bool) -> Element<'_, Msg> {
        let recentes = &self.config.recentes;
        let mut titulo = row![text(tr("Recentes")).size(16).font(negrito()), Space::new(Length::FILL, 0.0)].spacing(10).align(Align::Center);
        if !recentes.is_empty() {
            titulo = titulo.push(container(row![
                feather_ui::icones::icone("search", 15.0),
                text_input(tr("Buscar nos recentes"), &self.inicio.busca)
                    .on_input(|t| Msg::Inicio(Evento::Buscar(t)))
                    .size(13).padding([5, 4])
                    .style(|tema, estado| {
                        let mut e = pelicano::input_default(tema, estado);
                        e.background = Some(Background::Color(COR_PAINEL()));
                        e.border = Border::default();
                        e
                    }),
            ].spacing(6).align(Align::Center))
                .padding([1, 10])
                .width(Length::Fixed((largura * 0.35).clamp(170.0, 280.0)))
                .style(|_| Style {
                    background: Some(Background::Color(COR_PAINEL())),
                    border: Border { color: COR_BORDA(), width: 1.0, radius: [8.0; 4] },
                    ..Default::default()
                }));
        }
        if !todos && recentes.len() > 6 {
            titulo = titulo.push(botao_link(tr("Ver todos").into(), "chevron-right", Msg::Inicio(Evento::Pagina(PaginaMenu::Recentes))));
        }
        if recentes.is_empty() {
            return column![
                titulo,
                container(column![
                    feather_ui::icones::icone("folder-open", 30.0),
                    text(tr("Nenhum documento aberto ainda")).size(13),
                    text(tr("Os documentos que você abrir ou salvar aparecem aqui.")).size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(6).align(Align::Center))
                    .width(Length::FILL).align_x(Align::Center).padding(28),
            ].spacing(10).into();
        }
        let busca = self.inicio.busca.trim().to_lowercase();
        let achados: Vec<(usize, &PathBuf)> = recentes.iter().enumerate()
            .filter(|(_, p)| busca.is_empty() || p.to_string_lossy().to_lowercase().contains(&busca))
            .take(if todos { usize::MAX } else { 6 })
            .collect();
        let rotulo = |s: &str| text(tr(s).to_owned()).size(11).color(COR_TEXTO_MUTED());
        let colunas = container(row![
            Space::new(Length::Fixed(COL_TIPO), 0.0),
            container(rotulo("Nome")).width(Length::Fill(5)),
            container(rotulo("Pasta")).width(Length::Fill(4)),
            container(rotulo("Modificado")).width(Length::Fixed(COL_QUANDO)),
            Space::new(Length::Fixed(COL_ACOES + 4.0), 0.0),
        ].spacing(10)).padding([0, 10]);
        let mut lista = column![colunas, divisor()].spacing(4);
        if achados.is_empty() {
            lista = lista.push(container(text(trf("Nenhum recente com “{}”.", &[&self.inicio.busca.trim()])).size(12).color(COR_TEXTO_MUTED())).padding([14, 10]));
        }
        for (i, p) in achados {
            lista = lista.push(self.linha_recente(i, p, largura));
        }
        column![titulo, lista].spacing(10).into()
    }

    /// Marca e Abrir, a faixa "Novo" com os modelos e os recentes.
    fn inicio_conteudo(&self) -> Element<'_, Msg> {
        let cheia = self.inicio.tela_cheia;
        let largura = self.largura_inicio();
        let cabecalho = row![
            feather_ui::marca::icone_atual(if cheia { 44.0 } else { 36.0 }),
            column![
                text("FeatherDocs").size(if cheia { 24 } else { 20 }).font(negrito()),
                text(tr("Documentos que ficam na sua máquina.")).size(12.5).color(COR_TEXTO_MUTED()),
            ].spacing(1),
            Space::new(Length::FILL, 0.0),
            button(row![feather_ui::icones::icone("folder-open", 16.0), text(tr("Abrir…")).size(13)].spacing(7).align(Align::Center))
                .padding([8, 16]).style(estilo_botao_secundario).on_press(Msg::Comando(Comando::Abrir)),
        ].spacing(12).align(Align::Center);
        let faixa = container(self.galeria(largura - 36.0)).padding([14, 18]).width(Length::FILL).style(|_| Style {
            background: Some(Background::Color(COR_PAINEL_FAIXA())),
            border: Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [10.0; 4] },
            ..Default::default()
        });
        let recentes = self.recentes(largura, cheia);
        if cheia {
            // Só os recentes rolam; marca e modelos ficam fixos.
            return column![
                cabecalho,
                faixa,
                rolagem(container(recentes).height(Length::Shrink).padding([0, 2])).height(Length::FILL),
            ].spacing(18).width(Length::Fixed(largura)).height(Length::FILL).into();
        }
        column![cabecalho, faixa, recentes].spacing(16).into()
    }

    /// Tela inicial na janela inteira, enquanto nenhum documento foi escolhido.
    pub(crate) fn view_tela_inicio(&self) -> Element<'_, Msg> {
        container(self.inicio_conteudo())
            .padding([24, 44])
            .width(Length::FILL).align_x(Align::Center)
            .height(Length::FILL)
            .style(|_| Style { background: Some(Background::Color(COR_FUNDO_JANELA())), ..Default::default() })
            .into()
    }

    fn item_lateral(&self, rotulo: &str, icone: &'static str, m: Option<Msg>, selecionado: bool) -> Element<'_, Msg> {
        let habilitado = m.is_some();
        button(row![feather_ui::icones::icone(icone, 18.0), text(tr(rotulo).to_owned()).size(13), Space::new(Length::FILL, 0.0)].spacing(11).align(Align::Center))
            .width(Length::FILL).padding([9, 13])
            .style(move |tema, estado| {
                let mut e = estilo_botao_secundario(tema, estado);
                e.border.radius = [9.0; 4];
                e.border.width = 0.0;
                if selecionado {
                    e.background = Some(Background::Color(COR_AZUL_FUNDO()));
                    e.text_color = Some(COR_AZUL_ACENTO());
                } else if !matches!(estado, Status::Hovered) {
                    e.background = None;
                }
                if !habilitado {
                    e.text_color = Some(COR_TEXTO_MUTED());
                }
                e
            })
            .on_press_maybe(m)
            .into()
    }

    fn lateral(&self) -> Element<'_, Msg> {
        let p = self.inicio.pagina;
        let pag = |x| Some(Msg::Inicio(Evento::Pagina(x)));
        let c = |c| Some(Msg::Comando(c));
        let lista = column![
            self.item_lateral("Início", "layout-sidebar", pag(PaginaMenu::Inicio), p == PaginaMenu::Inicio),
            self.item_lateral("Novo", "file-plus", c(Comando::Novo), false),
            self.item_lateral("Abrir", "folder-open", c(Comando::Abrir), false),
            self.item_lateral("Recentes", "clock", pag(PaginaMenu::Recentes), p == PaginaMenu::Recentes),
            divisor(),
            self.item_lateral("Salvar", "device-floppy", c(Comando::Salvar), false),
            self.item_lateral("Salvar como…", "device-floppy", c(Comando::SalvarComo), false),
            divisor(),
            self.item_lateral("Exportar", "file-export", pag(PaginaMenu::Exportar), p == PaginaMenu::Exportar),
            self.item_lateral("Imprimir / PDF", "printer", c(Comando::ExportarPdf), false),
            divisor(),
            self.item_lateral("Informações", "info-circle", pag(PaginaMenu::Informacoes), p == PaginaMenu::Informacoes),
            self.item_lateral("Primeiros passos", "rocket", Some(Msg::BoasVindas(crate::boas_vindas::Evento::AbrirPassos)), false),
            self.item_lateral("O que há de novo", "sparkles", Some(Msg::BoasVindas(crate::boas_vindas::Evento::AbrirNovidades)), false),
            self.item_lateral("Opções", "settings", c(Comando::Configuracoes), false),
            divisor(),
            self.item_lateral("Fechar menu", "x", Some(Msg::Cancelar), false),
        ].spacing(3).padding(11);
        container(scrollable(lista).height(Length::FILL))
            .width(Length::Fixed(LATERAL)).height(Length::FILL)
            .style(|_| Style { background: Some(Background::Color(COR_PAINEL_FAIXA())), ..Default::default() })
            .into()
    }

    /// Página Exportar: um cartão por formato, cada um abre o diálogo de salvar.
    fn exportar(&self) -> Element<'_, Msg> {
        let cartao = |icone: &'static str, titulo: &str, extensao: &'static str, descricao: &str, m: Msg| -> Element<'_, Msg> {
            let simbolo = container(feather_ui::icones::icone(icone, 22.0)).padding(11).style(|_| Style {
                background: Some(Background::Color(COR_AZUL_FUNDO())),
                border: Border { radius: [10.0; 4], ..Default::default() },
                ..Default::default()
            });
            button(row![
                simbolo,
                column![
                    row![text(tr(titulo).to_owned()).size(13.5).font(negrito()), text(extensao).size(12).color(COR_TEXTO_MUTED())].spacing(6).align(Align::Center),
                    text(tr(descricao).to_owned()).size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(3).width(Length::FILL),
                feather_ui::icones::icone("chevron-right", 16.0),
            ].spacing(12).align(Align::Center))
                .width(Length::FILL).padding([11, 13])
                .style(|_, estado| {
                    let sobre = matches!(estado, Status::Hovered);
                    Style {
                        background: Some(Background::Color(if sobre { COR_AZUL_FUNDO() } else { COR_PAINEL() })),
                        text_color: Some(COR_TEXTO()),
                        border: Border { color: if sobre { COR_AZUL_ACENTO() } else { COR_BORDA() }, width: 1.0, radius: [11.0; 4] },
                        ..Default::default()
                    }
                })
                .on_press(m)
                .into()
        };
        column![
            text(tr("Exportar")).size(24).font(negrito()),
            text(tr("Salva uma cópia em outro formato. O .fdoz em que você trabalha não muda.")).size(12).color(COR_TEXTO_MUTED()),
            cartao("file-export", "Word, LaTeX ou Markdown", ".docx · .tex · .md", "Escolha o formato no diálogo de salvar; o que não cabe nele é avisado antes.", Msg::Comando(Comando::Exportar)),
            cartao("printer", "PDF", ".pdf", "Para imprimir ou mandar: páginas como na tela, com as opções do PDF.", Msg::Comando(Comando::ExportarPdf)),
        ].spacing(12).into()
    }

    fn informacoes(&self) -> Element<'_, Msg> {
        let e = self.sessao.estado();
        let nome = self.caminho.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str())
            .map_or_else(|| tr("Documento sem título").to_string(), str::to_string);
        column![
            text(tr("Informações")).size(24).font(negrito()),
            text(nome).size(17),
            text(self.caminho.as_ref().map_or_else(|| tr("Ainda não salvo em .fdoz").to_string(), |p| p.display().to_string())).size(12).color(COR_TEXTO_MUTED()),
            divisor(),
            text(trf("{} páginas · {} palavras · {} caracteres", &[&e.paginas, &e.palavras, &e.caracteres])).size(13),
            text(tr(if e.sujo { "Há alterações não salvas" } else { "Sem alterações pendentes" })).size(13),
            row![
                button(text(tr("Propriedades…")).size(12.5)).padding([7, 14]).style(estilo_botao_secundario)
                    .on_press(Msg::FormAbrir(crate::dialogos::Tipo::Propriedades)),
                button(text(tr("Salvar")).size(12.5)).padding([7, 14]).style(estilo_botao_primario).on_press(Msg::Comando(Comando::Salvar)),
            ].spacing(8),
        ].spacing(13).into()
    }

    fn conteudo_backstage(&self) -> Element<'_, Msg> {
        match self.inicio.pagina {
            PaginaMenu::Inicio => self.inicio_conteudo(),
            PaginaMenu::Recentes => column![
                text(tr("Recentes")).size(24).font(negrito()),
                text(tr("Abra ou tire arquivos da lista; ela fica só neste computador.")).size(12).color(COR_TEXTO_MUTED()),
                self.recentes(self.largura_inicio(), true),
            ].spacing(12).into(),
            PaginaMenu::Exportar => self.exportar(),
            PaginaMenu::Informacoes => self.informacoes(),
        }
    }

    /// O menu Arquivos por cima da faixa e da página, abaixo das abas; nada encolhe. Clicar fora fecha.
    pub(crate) fn sobrepor_backstage<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let janela = if self.tamanho_principal.width > 0.0 { self.tamanho_principal } else { pelicano::Size::new(1200.0, 800.0) };
        let largura = (janela.width - 20.0).min(960.0).max(310.0);
        let y = if self.config.barra_propria { 44.0 } else { 8.0 };
        let altura = (janela.height - y - 12.0).max(175.0);
        let painel = container(row![
            self.lateral(),
            container(scrollable(column![self.conteudo_backstage()].padding([23, 23])).height(Length::FILL))
                .width(Length::FILL).height(Length::FILL),
        ].height(Length::FILL))
            .width(Length::Fixed(largura)).height(Length::Fixed(altura))
            .style(|_| Style {
                background: Some(Background::Color(COR_PAINEL())),
                border: Border { color: COR_BORDA(), width: 1.0, radius: [13.0; 4] },
                shadow: Some(Shadow { color: Color::rgb8(0, 0, 0).alpha(0.22), offset: Point::new(0.0, 6.0), blur: 24.0 }),
                ..Default::default()
            });
        let fora = mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(Msg::Cancelar);
        stack![
            base,
            fora,
            column![Space::new(0.0, Length::Fixed(y)), row![Space::new(Length::Fixed(12.0), 0.0), painel]],
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_civil_e_miniaturas_de_todos_os_modelos() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_754), (2026, 10, 28));
        let inicio = Inicio::default();
        assert_eq!(inicio.miniaturas.len(), modelos::lista().len(), "todo modelo tem miniatura");
    }
}

#[cfg(test)]
mod testes_app {
    use crate::app::{App, Comando, Msg};

    #[test]
    fn modelo_cria_documento_e_sai_da_tela_inicial_e_a_estrutura_arrasta_ate_sumir() {
        let mut app = App::default();
        app.inicio.tela_cheia = true;
        let _ = app.view(pelicano::WindowId::MAIN);
        let _ = app.update(Msg::Inicio(super::Evento::Modelo("relatorio")));
        assert!(!app.inicio.tela_cheia);
        assert_eq!(app.sessao.documento().meta.titulo, "Relatório de atividades");
        let _ = app.update(Msg::Comando(Comando::Menu));
        assert!(app.menu_aberto);
        let _ = app.view(pelicano::WindowId::MAIN);

        app.config.estrutura = true;
        let _ = app.update(Msg::EstruturaArrastar);
        let _ = app.update(Msg::EstruturaMover(330.0));
        assert_eq!(app.config.largura_estrutura, 330.0);
        let _ = app.update(Msg::EstruturaMover(20.0));
        assert!(!app.config.estrutura, "encostar no canto fecha o painel");
        let _ = app.update(Msg::EstruturaMover(200.0));
        assert!(app.config.estrutura, "voltar sem soltar abre de novo");
        let _ = app.update(Msg::EstruturaSoltar);
        let _ = app.update(Msg::EstruturaMover(20.0));
        assert!(app.config.estrutura, "solto, mover o mouse não mexe no painel");
    }
}
