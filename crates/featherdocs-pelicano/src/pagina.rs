//! Páginas no pelicano: desenha o quadro e repassa coordenadas e teclas ao motor.
use crate::{
    app::Msg,
    quadro::{Operacao, Quadro},
};
use featherdocs_core::{geometria::Visao, medida, modelo::*};
use crate::compat::{canvas::{self, Frame, Path, Stroke, Text}, keyboard::{self, Modificadores as _}, mouse, Distancia as _, Vector};
use pelicano::{Captured, Color, Event, Font, Point, Rect, Renderer, Size, Theme};
use std::collections::HashMap;

pub struct Pagina<'a> {
    pub quadro: &'a Quadro,
    pub visao: &'a Visao,
    pub numero: usize,
    pub escala: f32,
    pub ativa: bool,
    pub imagens: &'a HashMap<String, pelicano::Image>,
    /// A seleção começou noutra página e o botão ainda está apertado.
    pub arrastando: bool,
    /// As tabelas da página (`Sessao::tabelas_na_pagina`): os botões de + e − nos cantos.
    pub tabelas: Vec<(u64, featherdocs_core::geometria::Ret, bool, bool)>,
    /// As imagens soltas da página (`Sessao::imagens_soltas_na_pagina`), a selecionada e, no arrasto,
    /// onde ela cairia nesta página.
    pub soltas: Vec<(u64, featherdocs_core::geometria::Ret, (f32, f32))>,
    pub solta: Option<u64>,
    pub fantasma: Option<Rect>,
    /// A figura selecionada: o contorno e as oito alças de redimensionar.
    pub alcas: Option<Rect>,
    /// Editando o cabeçalho ou o rodapé na página: o cursor do texto some e, nesta página, o da faixa aparece.
    pub editando_faixa: bool,
    pub cursor_faixa: Option<(f32, f32, f32)>,
}
#[derive(Default)]
pub struct Estado {
    arrastando: bool,
    modificadores: keyboard::Modifiers,
    clique: Option<(std::time::Instant, Point, u8)>,
}
pub fn cor(n: u32) -> Color {
    Color::rgb8((n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8).alpha((n & 255) as f32 / 255.)
}
/// Coordenadas do conteúdo Docs continuam em pontos. Não aplicar DPR
/// duas vezes: o mouse do pelicano vem em pixels lógicos.
fn ponto_da_pagina(p: Point, escala: f32) -> Point {
    let Ok(v) = feather_draw::Viewport::de_escala_logica(
        feather_draw::Unidade::Ponto,
        escala,
        feather_draw::Ponto::novo(0.0, 0.0),
        feather_draw::Ponto::novo(0.0, 0.0),
        1.0,
    ) else { return Point::new(0.0, 0.0); };
    let q=v.documento(feather_draw::Ponto::novo(p.x,p.y));
    Point::new(q.x,q.y)
}

const BOTAO: f32 = 14.;

/// Os botões da tabela sob o mouse `q` (pontos da página), como no FeatherNotes: + e − de coluna
/// no canto de cima à direita, + e − de linha no de baixo à esquerda. A ação é a de
/// `featherdocs_tabelas::docs_editar_estrutura`, na última coluna ou linha.
fn botoes(tabelas: &[(u64, featherdocs_core::geometria::Ret, bool, bool)], q: Point) -> Vec<(Rect, u64, &'static str)> {
    let mut v = Vec::new();
    for &(id, r, comeca, termina) in tabelas {
        let zona = Rect::new(Point::new(r.x, r.y), Size::new(r.w + BOTAO + 8., r.h + BOTAO + 8.));
        if !zona.contains(q) {
            continue;
        }
        let quadrado = |x, y| Rect::new(Point::new(x, y), Size::new(BOTAO, BOTAO));
        if comeca {
            v.push((quadrado(r.x + r.w + 4., r.y), id, "coluna+"));
            v.push((quadrado(r.x + r.w + 4., r.y + BOTAO + 3.), id, "coluna-"));
        }
        if termina {
            v.push((quadrado(r.x, r.y + r.h + 4.), id, "linha+"));
            v.push((quadrado(r.x + BOTAO + 3., r.y + r.h + 4.), id, "linha-"));
        }
    }
    v
}

fn fonte(estilo: u8) -> Font {
    Font {
        family: Some(if estilo & CODIGO != 0 {
            "Liberation Mono"
        } else if estilo & SERIFA != 0 {
            "Liberation Serif"
        } else {
            "Liberation Sans"
        }),
        weight: if estilo & NEGRITO != 0 { 700 } else { 400 },
        italic: estilo & ITALICO != 0,
    }
}
/// Leva o contorno de um glifo do `feather-math` para o caminho do canvas.
struct Contorno<'a>(&'a mut canvas::path::Builder);

impl feather_math::Contorno for Contorno<'_> {
    fn mover(&mut self, x: f32, y: f32) {
        self.0.move_to(Point::new(x, y));
    }
    fn linha(&mut self, x: f32, y: f32) {
        self.0.line_to(Point::new(x, y));
    }
    fn quadratica(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quadratic_curve_to(Point::new(x1, y1), Point::new(x, y));
    }
    fn cubica(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.bezier_curve_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y));
    }
    fn fechar(&mut self) {
        self.0.close();
    }
}

/// Uma fórmula: o motor compõe o LaTeX (o mesmo resultado que mediu a linha) e cada glifo da
/// fonte matemática vira um caminho preenchido; barras e traços, retângulos.
fn formula(f: &mut Frame, latex: &str, x: f32, y: f32, tamanho: f32, estilo: u8, c: Color) {
    let fm = feather_math::compor_guardado(latex, medida::modo_da_formula(estilo), tamanho);
    let cor_de = |n: u32| if n == 0 { c } else { cor(n) };
    for g in &fm.glifos {
        let caminho = Path::new(|b| feather_math::contorno(g.id, g.tamanho, x + g.x, y + g.y, &mut Contorno(b)));
        f.fill(&caminho, cor_de(g.cor));
        // Os traços finos da Latin Modern somem na tela (sem "hinting"): um contorno de 0,15 pt
        // faz o papel do reforço que o texto comum recebe do rasterizador de fontes.
        f.stroke(&caminho, Stroke::default().with_color(cor_de(g.cor)).with_width(0.15));
    }
    for r in &fm.retangulos {
        f.fill_rectangle(Point::new(x + r.x, y + r.y), Size::new(r.largura, r.altura), cor_de(r.cor));
    }
}

fn texto(f: &mut Frame, s: &str, x: f32, y: f32, tamanho: f32, estilo: u8, c: Color) {
    if estilo & MATEMATICA != 0 {
        formula(f, s, x, y, tamanho, estilo, c);
        return;
    }
    // Os mesmos avanços do motor (largura e kerning de cada par): cursor, seleção e quebra coincidem.
    let mut dx = 0.;
    let mut letras = s.chars().peekable();
    while let Some(ch) = letras.next() {
        f.fill_text(Text {
            content: ch.to_string(),
            position: Point::new(x + dx, y - tamanho * 0.84),
            color: c,
            size: tamanho.into(),
            font: fonte(estilo),
            line_height: 1.2,
            ..Text::default()
        });
        let par = letras.peek().map_or(0, |&b| medida::kerning(ch, b, estilo));
        dx += (f32::from(medida::largura_do_caractere(ch, estilo)) + f32::from(par)) * tamanho / 1000.;
    }
    for (bit, dy) in [(SUBLINHADO, 0.12), (TACHADO, -0.3)] {
        if estilo & bit != 0 {
            f.stroke(
                &Path::line(
                    Point::new(x, y + tamanho * dy),
                    Point::new(x + dx, y + tamanho * dy),
                ),
                Stroke::default()
                    .with_color(c)
                    .with_width((tamanho / 16.).max(0.5)),
            );
        }
    }
}
impl canvas::Program<Msg> for Pagina<'_> {
    type State = Estado;
    fn update(&self, s: &mut Estado, event: &Event, bounds: Rect, cursor: Option<Point>, publicar: &mut dyn FnMut(Msg)) -> Captured {
        // O ponto relativo ao canto da página, se o mouse está em cima dela (o `position_in` do Iced).
        let dentro = cursor.filter(|p| bounds.contains(*p)).map(|p| Point::new(p.x - bounds.x, p.y - bounds.y));
        if let (Event::MousePressed(mouse::Button::Left), Some(p)) = (event, dentro) {
            let q = ponto_da_pagina(p, self.escala);
            if let Some(&(_, id, acao)) = botoes(&self.tabelas, q).iter().find(|(r, ..)| r.contains(q)) {
                publicar(Msg::TabelaCanto(id, acao));
                return Captured::Yes;
            }
        }
        match event {
            Event::ModifiersChanged(m) => s.modificadores = *m,
            // Ctrl+roda: zoom; capturar impede que a rolagem da página ande junto.
            Event::Wheel { dy, modifiers, .. } if modifiers.control() && dentro.is_some() => {
                // A roda chega em pixels (48 por dente): 10% por dente; touchpad, proporcional.
                let passo = (dy * 0.0025).clamp(-0.1, 0.1);
                if passo != 0. {
                    publicar(Msg::Comando(crate::app::Comando::Zoom(passo)));
                }
                return Captured::Yes;
            }
            Event::MousePressed(mouse::Button::Right) => {
                if let (Some(p), Some(abs)) = (dentro, cursor) {
                    let q = ponto_da_pagina(p, self.escala);
                    publicar(Msg::BotaoDireito(self.numero, q.x, q.y, abs));
                    return Captured::Yes;
                }
            }
            Event::MousePressed(mouse::Button::Left) if s.modificadores.control() => {
                if let Some(p) = dentro {
                    let q = ponto_da_pagina(p, self.escala);
                    publicar(Msg::CtrlClique(self.numero, q.x, q.y));
                    return Captured::Yes;
                }
            }
            Event::MousePressed(mouse::Button::Left) => {
                if let Some(p) = dentro {
                    let agora = std::time::Instant::now();
                    let n = s
                        .clique
                        .filter(|(t, a, _)| {
                            agora.duration_since(*t).as_millis() < 450 && p.distance(*a) < 5.
                        })
                        .map_or(1, |(_, _, n)| (n % 3) + 1);
                    s.clique = Some((agora, p, n));
                    s.arrastando = true;
                    let q = ponto_da_pagina(p, self.escala);
                    publicar(Msg::Clicar(self.numero, q.x, q.y, s.modificadores.shift(), n));
                    return Captured::Yes;
                }
            }
            Event::CursorMoved(_) if s.arrastando || self.arrastando => {
                if let Some(p) = dentro {
                    let q = ponto_da_pagina(p, self.escala);
                    publicar(Msg::Clicar(self.numero, q.x, q.y, true, 1));
                    return Captured::Yes;
                }
            }
            Event::MouseReleased(mouse::Button::Left) if s.arrastando => {
                s.arrastando = false;
                publicar(Msg::Soltar);
            }
            Event::Key { key, modifiers, text, pressed: true } if self.ativa => {
                // Atalhos de arquivo e clipboard ficam no `evento_janela` do aplicativo.
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(c) if ["n", "o", "s", "p", "c", "x", "v", "f"].contains(&c.to_lowercase().as_str()))
                {
                    return Captured::No;
                }
                publicar(Msg::Tecla(key.clone(), *modifiers, text.clone()));
                return Captured::Yes;
            }
            _ => {}
        }
        Captured::No
    }
    fn draw(&self, _: &Estado, renderer: &mut Renderer, bounds: Rect, _: &Theme, mouse: Option<Point>) {
        let mut f = Frame::new(renderer, bounds);
        f.scale(self.escala);
        f.fill_rectangle(
            Point::ORIGIN,
            Size::new(self.quadro.largura, self.quadro.altura),
            Color::WHITE,
        );
        for o in &self.quadro.operacoes {
            // O canvas e o SVG recebem a mesma geometria neutra Drawing, sem
            // converter medidas em pontos para pixels ou alterar z-order.
            if crate::desenho_compartilhado::desenhar_vetor(&mut f, o) {
                continue;
            }
            match o {
                Operacao::Retangulo { .. } | Operacao::Traco { .. } => {},
                Operacao::Texto {
                    x,
                    y,
                    tamanho,
                    estilo,
                    cor: c,
                    texto: s,
                    angulo,
                } => {
                    if let Some(a) = angulo {
                        f.with_save(|f| {
                            f.translate(Vector::new(*x, *y));
                            f.rotate(-a.to_radians());
                            texto(
                                f,
                                s,
                                -medida::largura(s, *estilo, *tamanho) / 2.,
                                tamanho * 0.3,
                                *tamanho,
                                *estilo,
                                cor(*c),
                            );
                        });
                    } else {
                        texto(&mut f, s, *x, *y, *tamanho, *estilo, cor(*c));
                    }
                }
                Operacao::Imagem { rect, origem, ajuste } => {
                    // As áreas do Docs continuam em pontos. A geometria é
                    // validada pelo Drawing antes de compor a imagem.
                    if let Some(area) = crate::imagem_compartilhada::area_desenho(*rect) {
                        let posicao = Point::new(area.x, area.y);
                        let tamanho = Size::new(area.largura, area.altura);
                        let graus = ajuste.map_or(0.0, |a| a[4]);
                        if let Some(imagem) = self.imagens.get(&crate::imagem_compartilhada::chave_da_imagem(origem, *ajuste)) {
                            if graus == 0.0 {
                                f.draw_image(Rect::new(posicao, tamanho), imagem);
                            } else {
                                // Girada em volta do centro, no sentido horário (o `rot` do Word).
                                f.with_save(|f| {
                                    f.translate(Vector::new(posicao.x + tamanho.width / 2., posicao.y + tamanho.height / 2.));
                                    f.rotate(graus.to_radians());
                                    f.draw_image(Rect::new(Point::new(-tamanho.width / 2., -tamanho.height / 2.), tamanho), imagem);
                                });
                            }
                        } else {
                            f.fill_rectangle(posicao, tamanho, Color::rgb8(235, 235, 235));
                        }
                    }
                }
            }
        }
        for r in self
            .visao
            .achados
            .iter()
            .chain(&self.visao.retangulos)
            .filter(|r| r.pagina == self.numero)
        {
            f.fill_rectangle(
                Point::new(r.x, r.y),
                Size::new(r.w, r.h),
                Color::rgba(0.2, 0.5, 1., 0.22),
            );
        }
        // A palavra errada: a linha ondulada vermelha embaixo dela, como nos outros editores.
        for r in self.visao.ortografia.iter().filter(|r| r.pagina == self.numero) {
            let y = r.y + r.h - 1.5;
            let ondas = Path::new(|b| {
                b.move_to(Point::new(r.x, y));
                let mut x = r.x;
                let mut cima = true;
                while x < r.x + r.w {
                    x = (x + 1.5).min(r.x + r.w);
                    b.line_to(Point::new(x, if cima { y - 1.2 } else { y }));
                    cima = !cima;
                }
            });
            f.stroke(&ondas, Stroke::default().with_color(Color::rgb8(0xd9, 0x30, 0x25)).with_width(0.8));
        }
        if self.ativa && !self.editando_faixa {
            if let Some(c) = self.visao.cursor.filter(|c| c.pagina == self.numero) {
                f.fill_rectangle(Point::new(c.x, c.y), Size::new(1., c.h), Color::BLACK);
            }
        }
        if let Some((x, y, h)) = self.cursor_faixa {
            f.fill_rectangle(Point::new(x, y), Size::new(1., h), Color::BLACK);
        }
        let sob = mouse.filter(|p| bounds.contains(*p)).map(|p| ponto_da_pagina(Point::new(p.x - bounds.x, p.y - bounds.y), self.escala));
        for (r, _, acao) in sob.map(|q| botoes(&self.tabelas, q)).unwrap_or_default() {
            let acesa = sob.is_some_and(|q| r.contains(q));
            let fio = Color::rgb8(0x2c, 0x75, 0xab);
            f.fill(&Path::rounded_rectangle(Point::new(r.x, r.y), r.size(), 3.), if acesa { fio } else { Color::WHITE });
            f.stroke(&Path::rounded_rectangle(Point::new(r.x, r.y), r.size(), 3.), Stroke::default().with_color(fio).with_width(1.));
            let (c, m) = (Point::new(r.x + BOTAO / 2., r.y + BOTAO / 2.), BOTAO * 0.28);
            let traco = Stroke::default().with_color(if acesa { Color::WHITE } else { fio }).with_width(1.5);
            f.stroke(&Path::line(Point::new(c.x - m, c.y), Point::new(c.x + m, c.y)), traco.clone());
            if acao.ends_with('+') {
                f.stroke(&Path::line(Point::new(c.x, c.y - m), Point::new(c.x, c.y + m)), traco);
            }
        }
        for (id, r, (ax, ay)) in &self.soltas {
            if self.solta != Some(*id) {
                continue;
            }
            let azul = Color::rgb8(0x2c, 0x75, 0xab);
            f.stroke(&Path::rectangle(Point::new(r.x, r.y), Size::new(r.w, r.h)), Stroke::default().with_color(azul).with_width(1.));
            // A âncora do Word, na margem do parágrafo que segura a imagem.
            let (x, y) = (ax - 10., ay + 1.);
            let traco = Stroke::default().with_color(azul).with_width(1.2);
            let mut ancora = canvas::Builder::new();
            ancora.circle(Point::new(x, y + 1.5), 1.5);
            ancora.move_to(Point::new(x, y + 3.));
            ancora.line_to(Point::new(x, y + 12.));
            ancora.move_to(Point::new(x - 3., y + 5.5));
            ancora.line_to(Point::new(x + 3., y + 5.5));
            ancora.move_to(Point::new(x - 5., y + 9.));
            ancora.line_to(Point::new(x, y + 12.));
            ancora.line_to(Point::new(x + 5., y + 9.));
            f.stroke(&ancora.build(), traco);
        }
        if let Some(r) = self.alcas {
            let azul = Color::rgb8(0x2c, 0x75, 0xab);
            f.stroke(&Path::rectangle(Point::new(r.x, r.y), r.size()), Stroke::default().with_color(azul).with_width(1. / self.escala));
            let lado = 7. / self.escala;
            for (x, y) in crate::app::alcas_da_figura([r.x, r.y, r.width, r.height]) {
                let canto = Point::new(x - lado / 2., y - lado / 2.);
                f.fill_rectangle(canto, Size::new(lado, lado), Color::WHITE);
                f.stroke(&Path::rectangle(canto, Size::new(lado, lado)), Stroke::default().with_color(azul).with_width(1. / self.escala));
            }
        }
        if let Some(r) = self.fantasma {
            f.fill_rectangle(Point::new(r.x, r.y), r.size(), Color::rgba(0.2, 0.5, 1., 0.15));
            f.stroke(&Path::rectangle(Point::new(r.x, r.y), r.size()), Stroke::default().with_color(Color::rgb8(0x2c, 0x75, 0xab)).with_width(1.));
        }
        f.fim();
    }
    fn mouse_interaction(&self, _: &Estado, bounds: Rect, cursor: Option<Point>) -> mouse::Interaction {
        let q = cursor.filter(|p| bounds.contains(*p)).map(|p| ponto_da_pagina(Point::new(p.x - bounds.x, p.y - bounds.y), self.escala));
        let na_solta = |q: Point| self.soltas.iter().any(|(_, r, _)| (r.x..=r.x + r.w).contains(&q.x) && (r.y..=r.y + r.h).contains(&q.y));
        if q.is_some_and(|q| botoes(&self.tabelas, q).iter().any(|(r, ..)| r.contains(q)) || na_solta(q)) {
            mouse::Interaction::Pointer
        } else if cursor.is_some_and(|p| bounds.contains(p)) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Apenas imagens embutidas; abrir um documento não acessa URLs nem caminhos externos.
/// Como [`carregar_imagem`], com o recorte e o espelho do Word já nos pixels.
pub fn carregar_imagem_ajustada(origem: &str, ajuste: Option<[f32; 7]>) -> Option<pelicano::Image> {
    match ajuste {
        Some(a) if !origem.starts_with("data:image/svg+xml;base64,") && crate::imagem_compartilhada::chave_da_imagem(origem, ajuste) != origem => {
            let (w, h, rgba) = crate::imagem_compartilhada::preparar_previa(origem)?;
            let (w, h, rgba) = crate::imagem_compartilhada::ajustar_rgba(w, h, rgba, a)?;
            pelicano::Image::from_rgba(w, h, rgba)
        }
        _ => carregar_imagem(origem),
    }
}

pub fn carregar_imagem(origem: &str) -> Option<pelicano::Image> {
    // PNG/JPEG passam uma única vez pelo Image neutro: limitar data URL
    // ANTES de base64 e devolver somente a miniatura RGBA ao pelicano.
    if !origem.starts_with("data:image/svg+xml;base64,") {
        let (w, h, rgba) = crate::imagem_compartilhada::preparar_previa(origem)?;
        return pelicano::Image::from_rgba(w, h, rgba);
    }
    pelicano::Image::from_png(&pixmap_svg(origem)?.encode_png().ok()?)
}

/// Rasteriza um gráfico SVG embutido (`data:image/svg+xml;base64,…`), com limite de entrada.
pub fn pixmap_svg(origem: &str) -> Option<resvg::tiny_skia::Pixmap> {
    if origem.len() > 16 * 1024 * 1024 {
        return None;
    }
    let (mime, bytes) = featherdocs_io::base64::de_data_url(origem)?;
    if mime != "image/svg+xml" {
        return None;
    }
    let mut opcoes = resvg::usvg::Options::default();
    opcoes.image_href_resolver.resolve_string = Box::new(|_, _| None);
    // O texto do SVG (desenho do TikZ, rótulos do gráfico) usa as fontes que o programa já leva.
    static FONTES: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    opcoes.fontdb = FONTES
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            for f in crate::FONTES_DO_TEXTO {
                db.load_font_data(f.to_vec());
            }
            db.set_serif_family("Liberation Serif");
            db.set_sans_serif_family("Liberation Sans");
            db.set_monospace_family("Liberation Mono");
            std::sync::Arc::new(db)
        })
        .clone();
    let tree = resvg::usvg::Tree::from_data(&bytes, &opcoes).ok()?;
    // 4 pixels por ponto (o fio de 0,4 pt fica preto, não cinza), até 4096 px no lado maior.
    let (w, h) = (tree.size().width(), tree.size().height());
    let fator = (4096.0 / w.max(h).max(1.0)).min(4.0).max(1.0);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(
        ((w * fator).ceil() as u32).clamp(1, 4096), ((h * fator).ceil() as u32).clamp(1, 4096),
    )?;
    let escala = (pixmap.width() as f32 / tree.size().width())
        .min(pixmap.height() as f32 / tree.size().height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(escala, escala),
        &mut pixmap.as_mut(),
    );
    Some(pixmap)
}

#[cfg(test)]
mod testes_viewport {
    use super::*;
    #[test]
    fn clique_em_pontos_preserva_escala_do_papel() {
        let p=ponto_da_pagina(Point::new(144.0,72.0),2.0);
        assert!((p.x-72.0).abs()<0.001);
        assert!((p.y-36.0).abs()<0.001);
        let p2=ponto_da_pagina(Point::new(96.0,48.0),96.0/72.0);
        assert!((p2.x-72.0).abs()<0.001);
        assert!((p2.y-36.0).abs()<0.001);
    }
}
