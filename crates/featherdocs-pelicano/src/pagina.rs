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
    // Mesmos avanços sem kerning que o motor: cursor, seleção e quebra coincidem.
    let mut dx = 0.;
    for ch in s.chars() {
        f.fill_text(Text {
            content: ch.to_string(),
            position: Point::new(x + dx, y - tamanho * 0.84),
            color: c,
            size: tamanho.into(),
            font: fonte(estilo),
            line_height: 1.2,
            ..Text::default()
        });
        dx += medida::largura_do_caractere(ch, estilo) as f32 * tamanho / 1000.;
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
    fn draw(&self, _: &Estado, renderer: &mut Renderer, bounds: Rect, _: &Theme, _: Option<Point>) {
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
                Operacao::Imagem { rect, origem } => {
                    // As áreas do Docs continuam em pontos. A geometria é
                    // validada pelo Drawing antes de compor a imagem.
                    if let Some(area) = crate::imagem_compartilhada::area_desenho(*rect) {
                        let posicao = Point::new(area.x, area.y);
                        let tamanho = Size::new(area.largura, area.altura);
                        if let Some(imagem) = self.imagens.get(origem) {
                            f.draw_image(Rect::new(posicao, tamanho), imagem);
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
        if self.ativa {
            if let Some(c) = self.visao.cursor.filter(|c| c.pagina == self.numero) {
                f.fill_rectangle(Point::new(c.x, c.y), Size::new(1., c.h), Color::BLACK);
            }
        }
        f.fim();
    }
    fn mouse_interaction(&self, _: &Estado, bounds: Rect, cursor: Option<Point>) -> mouse::Interaction {
        if cursor.is_some_and(|p| bounds.contains(p)) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Apenas imagens embutidas; abrir um documento não acessa URLs nem caminhos externos.
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
