//! Exportar PDF: as mesmas operações de página que a tela desenha. O texto sai nas fontes da tela
//! (Liberation, embutidas; as mesmas medidas de Helvetica, Times e Courier, então cai no mesmo
//! lugar) ou nas fontes padrão do PDF; as fórmulas saem como os contornos que a tela pinta.
//! Opções: páginas, marca d'água, dados do documento, links e marcadores.
use crate::localidade::{tr, trf};
use crate::quadro::{Operacao, Quadro};
use feather_draw::pdf::{DestinoPdf, DocumentoPdf, FonteEmbutida, FontePdf, MetadadosPdf, PaginaPdf};
use feather_draw::{Caminho, Comando, Cor, Ponto};
use featherdocs_core::medida;
use featherdocs_core::modelo::{CODIGO, ITALICO, MATEMATICA, NEGRITO, SERIFA, SUBLINHADO, TACHADO};
use featherdocs_core::quadro::{DestinoDoLink, LinkDaPagina, MarcadorDoTitulo};
use std::collections::HashMap;

/// O que a pessoa escolheu na janela Exportar PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct OpcoesPdf {
    /// Páginas como "1-3, 5" (contadas do 1); vazio = todas.
    pub paginas: String,
    /// Texto na diagonal de cada página ("RASCUNHO"); vazio = sem.
    pub marca: String,
    pub titulo: String,
    pub autor: String,
    pub assunto: String,
    pub palavras_chave: String,
    pub links: bool,
    pub marcadores: bool,
    /// As fontes da tela embutidas; sem elas, as fontes padrão do PDF (arquivo menor, só Latin-1).
    pub fontes_da_tela: bool,
    pub abrir_depois: bool,
    /// Só da janela: 0 todas, 1 a atual, 2 intervalo digitado (o campo fica à vista mesmo vazio).
    pub escolha_paginas: u8,
    /// Só da janela: marca d'água com texto próprio (o campo fica à vista mesmo vazio).
    pub marca_propria: bool,
}

impl Default for OpcoesPdf {
    fn default() -> Self {
        Self {
            paginas: String::new(),
            marca: String::new(),
            titulo: String::new(),
            autor: String::new(),
            assunto: String::new(),
            palavras_chave: String::new(),
            links: true,
            marcadores: true,
            fontes_da_tela: true,
            abrir_depois: false,
            escolha_paginas: 0,
            marca_propria: false,
        }
    }
}

impl OpcoesPdf {
    /// O padrão com título e autor do documento, numa linha (o `\\` do LaTeX vira espaço simples).
    pub fn do_documento(titulo: &str, autor: &str) -> Self {
        let linha = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
        Self { titulo: linha(titulo), autor: linha(autor), ..Default::default() }
    }
}

/// As páginas (contadas do 0) de "1-3, 5" num documento de `total` páginas; vazio = todas.
pub fn paginas_do_intervalo(texto: &str, total: usize) -> Result<Vec<usize>, String> {
    if texto.trim().is_empty() {
        return Ok((0..total).collect());
    }
    let mut saida = Vec::new();
    for parte in texto.split([',', ';']).map(str::trim).filter(|p| !p.is_empty()) {
        let numero = |s: &str| s.trim().parse::<usize>().ok().filter(|n| (1..=total).contains(n));
        let (a, b) = match parte.split_once('-') {
            Some((a, b)) => (numero(a), if b.trim().is_empty() { Some(total) } else { numero(b) }),
            None => (numero(parte), numero(parte)),
        };
        match (a, b) {
            (Some(a), Some(b)) if a <= b => saida.extend(a - 1..b),
            _ => return Err(trf("Páginas inválidas: \"{}\" (o documento tem {} páginas).", &[&parte, &total.to_string()])),
        }
    }
    saida.dedup();
    Ok(saida)
}

fn cor(c: u32) -> Cor {
    Cor::rgb((c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8)
}

fn fonte(estilo: u8) -> FontePdf {
    use FontePdf::*;
    let (n, i) = (estilo & NEGRITO != 0, estilo & ITALICO != 0);
    let familia = if estilo & CODIGO != 0 {
        [Mono, MonoNegrito, MonoItalico, MonoNegritoItalico]
    } else if estilo & SERIFA != 0 {
        [Serifa, SerifaNegrito, SerifaItalico, SerifaNegritoItalico]
    } else {
        [Helvetica, Negrito, Italico, NegritoItalico]
    };
    familia[usize::from(n) + 2 * usize::from(i)]
}

/// A fonte da tela de `estilo` em `FONTES_DO_TEXTO` (a mono não tem itálico: fica a reta).
fn fonte_da_tela(estilo: u8) -> usize {
    let (n, i) = (usize::from(estilo & NEGRITO != 0), usize::from(estilo & ITALICO != 0));
    if estilo & CODIGO != 0 {
        8 + n
    } else if estilo & SERIFA != 0 {
        4 + n + 2 * i
    } else {
        n + 2 * i
    }
}

/// O texto de uma página nas fontes escolhidas.
struct Letras {
    tela: bool,
    embutidas: HashMap<usize, FonteEmbutida>,
}

impl Letras {
    #[allow(clippy::too_many_arguments)]
    fn escrever(&mut self, doc: &mut DocumentoPdf, k: usize, (x, y, graus): (f32, f32, f32), texto: &str, estilo: u8, tamanho: f32, c: Cor) {
        if self.tela {
            let i = fonte_da_tela(estilo);
            let f = match self.embutidas.get(&i) {
                Some(f) => *f,
                None => match doc.fonte(crate::FONTES_DO_TEXTO[i].to_vec()) {
                    Ok(f) => *self.embutidas.entry(i).or_insert(f),
                    Err(_) => {
                        self.tela = false;
                        return self.escrever(doc, k, (x, y, graus), texto, estilo, tamanho, c);
                    }
                },
            };
            if let Some(p) = doc.pagina(k) {
                p.texto_embutido(x, y, graus, texto, f, tamanho, c);
            }
        } else if let Some(p) = doc.pagina(k) {
            if graus == 0.0 {
                p.texto(x, y, texto, fonte(estilo), tamanho, c);
            } else {
                p.texto_girado(x, y, graus, texto, fonte(estilo), tamanho, c);
            }
        }
    }
}

/// Os contornos de `feather_math` como caminho do PDF.
struct Contorno(Caminho);

impl feather_math::Contorno for Contorno {
    fn mover(&mut self, x: f32, y: f32) {
        self.0.adicionar(Comando::Mover(Ponto::novo(x, y)));
    }
    fn linha(&mut self, x: f32, y: f32) {
        self.0.adicionar(Comando::Linha(Ponto::novo(x, y)));
    }
    fn quadratica(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.adicionar(Comando::Quadratica { controle: Ponto::novo(x1, y1), fim: Ponto::novo(x, y) });
    }
    fn cubica(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.adicionar(Comando::Cubica { controle1: Ponto::novo(x1, y1), controle2: Ponto::novo(x2, y2), fim: Ponto::novo(x, y) });
    }
    fn fechar(&mut self) {
        self.0.adicionar(Comando::Fechar);
    }
}

/// A fórmula como a tela a pinta (`pagina::formula`): glifos da Latin Modern Math e os fios.
fn formula(p: &mut PaginaPdf, latex: &str, x: f32, y: f32, tamanho: f32, estilo: u8, c: u32) {
    let fm = feather_math::compor_guardado(latex, medida::modo_da_formula(estilo), tamanho);
    let cor_de = |n: u32| if n == 0 { cor(c) } else { cor(n) };
    for g in &fm.glifos {
        let mut caminho = Contorno(Caminho::novo());
        feather_math::contorno(g.id, g.tamanho, x + g.x, y + g.y, &mut caminho);
        if !caminho.0.comandos.is_empty() {
            p.caminho_preenchido(&caminho.0, cor_de(g.cor));
        }
    }
    for r in &fm.retangulos {
        if r.largura > 0.0 && r.altura > 0.0 {
            p.retangulo_preenchido(x + r.x, y + r.y, r.largura, r.altura, cor_de(r.cor));
        }
    }
}

/// O PDF das páginas como a tela as mostra, com as opções padrão (todas as páginas, fontes da tela).
pub fn gerar(quadros: &[Quadro]) -> Result<Vec<u8>, String> {
    gerar_com(quadros, &OpcoesPdf::default(), &[], &[])
}

/// O PDF com as `opcoes`; `links` e `marcadores` vêm do motor (`Sessao::links_do_pdf`, `marcadores_do_pdf`).
pub fn gerar_com(quadros: &[Quadro], opcoes: &OpcoesPdf, links: &[LinkDaPagina], marcadores: &[MarcadorDoTitulo]) -> Result<Vec<u8>, String> {
    let paginas = paginas_do_intervalo(&opcoes.paginas, quadros.len())?;
    // A página do documento → a página do PDF (as que ficaram de fora não recebem link).
    let nova: HashMap<usize, usize> = paginas.iter().enumerate().map(|(k, &p)| (p, k)).collect();
    let mut doc = DocumentoPdf::novo();
    // Caractere fora do WinAnsi (grego, símbolos) vira '?' nas fontes padrão em vez de falhar o arquivo todo.
    doc.substituir_nao_suportados(true);
    doc.metadados(MetadadosPdf {
        titulo: opcoes.titulo.clone(),
        autor: opcoes.autor.clone(),
        assunto: opcoes.assunto.clone(),
        palavras_chave: opcoes.palavras_chave.clone(),
        criador: "FeatherDocs".into(),
    });
    let mut letras = Letras { tela: opcoes.fontes_da_tela, embutidas: HashMap::new() };
    for (k, &n) in paginas.iter().enumerate() {
        let q = &quadros[n];
        doc.adicionar_pagina(q.largura, q.altura);
        if !opcoes.marca.trim().is_empty() {
            marca_dagua(&mut doc, &mut letras, k, q, opcoes.marca.trim());
        }
        for o in &q.operacoes {
            match o {
                Operacao::Retangulo { rect: [x, y, w, h], cor: c } => {
                    if c & 0xff != 0 && *w > 0.0 && *h > 0.0 {
                        if let Some(p) = doc.pagina(k) {
                            p.retangulo_preenchido(*x, *y, *w, *h, cor(*c));
                        }
                    }
                }
                Operacao::Traco { pontos: [x, y, a, b, largura], cor: c } => {
                    if c & 0xff != 0 && *largura > 0.0 {
                        if let Some(p) = doc.pagina(k) {
                            p.linha(*x, *y, *a, *b, cor(*c), *largura, false);
                        }
                    }
                }
                Operacao::Imagem { rect: [x, y, w, h], origem } => {
                    if let (Some((rgb, pw, ph)), Some(p)) = (rgb_da_imagem(origem), doc.pagina(k)) {
                        p.imagem_rgb(*x, *y, *w, *h, rgb, pw, ph);
                    }
                }
                Operacao::Texto { x, y, tamanho, estilo, cor: c, texto, angulo: None } if estilo & MATEMATICA != 0 => {
                    if let Some(p) = doc.pagina(k) {
                        formula(p, texto, *x, *y, *tamanho, *estilo, *c);
                    }
                }
                Operacao::Texto { x, y, tamanho, estilo, cor: c, texto, angulo: Some(a) } => {
                    // O mesmo ponto de partida da tela: centrado em (x, y), girado `a` graus.
                    let (ox, oy) = (-medida::largura(texto, *estilo, *tamanho) / 2., tamanho * 0.3);
                    let (sen, cos) = (-a).to_radians().sin_cos();
                    letras.escrever(&mut doc, k, (x + ox * cos - oy * sen, y + ox * sen + oy * cos, *a), texto, *estilo, *tamanho, cor(*c));
                }
                Operacao::Texto { x, y, tamanho, estilo, cor: c, texto, angulo: None } => {
                    letras.escrever(&mut doc, k, (*x, *y, 0.0), texto, *estilo, *tamanho, cor(*c));
                    let w = medida::largura(texto, *estilo, *tamanho);
                    for (bit, dy) in [(SUBLINHADO, 0.12), (TACHADO, -0.3)] {
                        if estilo & bit != 0 {
                            let yy = y + tamanho * dy;
                            if let Some(p) = doc.pagina(k) {
                                p.linha(*x, yy, x + w, yy, cor(*c), (tamanho / 16.).max(0.5), false);
                            }
                        }
                    }
                }
            }
        }
    }
    if opcoes.links {
        for l in links {
            let destino = match &l.destino {
                DestinoDoLink::Url(u) => DestinoPdf::Url(u.clone()),
                DestinoDoLink::Pagina(p, y) => match nova.get(p) {
                    Some(&indice) => DestinoPdf::Pagina { indice, y: *y },
                    None => continue,
                },
            };
            if let Some(p) = nova.get(&l.pagina).and_then(|&k| doc.pagina(k)) {
                let [x, y, w, h] = l.area;
                p.link(x, y, w, h, destino);
            }
        }
    }
    if opcoes.marcadores {
        for m in marcadores {
            if let Some(&k) = nova.get(&m.pagina) {
                doc.marcador(m.nivel, &m.texto, k, m.y);
            }
        }
    }
    doc.finalizar().map_err(|e| trf("Não consegui gerar o PDF: {}", &[&format!("{e:?}")]))
}

/// O texto em cinza-claro na diagonal da página, por baixo do conteúdo.
fn marca_dagua(doc: &mut DocumentoPdf, letras: &mut Letras, k: usize, q: &Quadro, texto: &str) {
    let graus = (q.altura / q.largura).atan().to_degrees();
    let diagonal = q.largura.hypot(q.altura);
    let estilo = NEGRITO;
    // Ocupa uns 70% da diagonal.
    let tamanho = (diagonal * 0.7 / medida::largura(texto, estilo, 1.0).max(0.01)).min(160.0);
    let (ox, oy) = (-medida::largura(texto, estilo, tamanho) / 2., tamanho * 0.3);
    let (sen, cos) = (-graus).to_radians().sin_cos();
    let (cx, cy) = (q.largura / 2.0, q.altura / 2.0);
    letras.escrever(doc, k, (cx + ox * cos - oy * sen, cy + ox * sen + oy * cos, graus), texto, estilo, tamanho, Cor::rgb(225, 225, 225));
}

/// Os pixels de uma imagem embutida em RGB sobre fundo branco (o PDF base não tem transparência).
fn rgb_da_imagem(origem: &str) -> Option<(Vec<u8>, u32, u32)> {
    if origem.starts_with("data:image/svg+xml;base64,") {
        let p = crate::pagina::pixmap_svg(origem)?;
        // Pixmap vem com alfa pré-multiplicado: sobre branco é cor + (255 − alfa).
        let rgb = p.data().chunks_exact(4).flat_map(|c| [0, 1, 2].map(|i| c[i].saturating_add(255 - c[3]))).collect();
        return Some((rgb, p.width(), p.height()));
    }
    let (w, h, rgba) = crate::imagem_compartilhada::preparar_previa(origem)?;
    let sobre_branco = |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 255 * u16::from(255 - a)) / 255) as u8;
    let rgb = rgba.chunks_exact(4).flat_map(|c| [0, 1, 2].map(|i| sobre_branco(c[i], c[3]))).collect();
    Some((rgb, w, h))
}

/// O que não sai no PDF (imagens que não abriram), para avisar a pessoa.
pub fn aviso(quadros: &[Quadro]) -> Option<String> {
    let imagens = quadros
        .iter()
        .flat_map(|q| &q.operacoes)
        .filter(|o| matches!(o, Operacao::Imagem { origem, .. } if rgb_da_imagem(origem).is_none()))
        .count();
    (imagens > 0).then(|| trf("PDF exportado sem: {}", &[&format!("{imagens} {}", tr("imagem(ns) que não abriram"))]))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn quadro() -> Quadro {
        Quadro {
            largura: 200.0,
            altura: 100.0,
            operacoes: vec![
                Operacao::Texto { x: 10.0, y: 20.0, tamanho: 12.0, estilo: SERIFA | NEGRITO, cor: 0x000000ff, texto: "Ação α".into(), angulo: None },
                Operacao::Texto { x: 10.0, y: 40.0, tamanho: 10.0, estilo: CODIGO | SUBLINHADO, cor: 0x000000ff, texto: "x".into(), angulo: None },
                Operacao::Texto { x: 10.0, y: 60.0, tamanho: 10.0, estilo: MATEMATICA, cor: 0x000000ff, texto: "\\frac{a}{b}".into(), angulo: None },
                Operacao::Imagem { rect: [0.0, 0.0, 10.0, 10.0], origem: String::new() },
                Operacao::Texto { x: 100.0, y: 50.0, tamanho: 40.0, estilo: 0, cor: 0x888888ff, texto: "RASCUNHO".into(), angulo: Some(45.0) },
            ],
        }
    }

    #[test]
    fn fontes_padrao_formula_vetorial_e_aviso_de_imagem() {
        let q = [quadro()];
        let opcoes = OpcoesPdf { fontes_da_tela: false, ..OpcoesPdf::default() };
        let pdf = gerar_com(&q, &opcoes, &[], &[]).unwrap();
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF"));
        assert!(s.contains("/BaseFont /Times-Bold") && s.contains("/BaseFont /Courier"));
        assert!(s.contains("0.7071 0.7071 -0.7071 0.7071"), "texto girado 45°");
        // A fórmula sai como contorno preenchido, não como texto.
        assert!(s.contains(" c\n") && s.contains("\nf\nQ\n"));
        assert!(aviso(&q).unwrap().contains("1 imagem"));
    }

    #[test]
    fn fontes_da_tela_paginas_links_marcadores_e_dados() {
        let q = [quadro(), quadro(), quadro()];
        let links = [
            LinkDaPagina { pagina: 0, area: [10.0, 10.0, 30.0, 12.0], destino: DestinoDoLink::Url("https://exemplo.org".into()) },
            LinkDaPagina { pagina: 2, area: [10.0, 10.0, 30.0, 12.0], destino: DestinoDoLink::Pagina(0, 20.0) },
            LinkDaPagina { pagina: 2, area: [10.0, 30.0, 30.0, 12.0], destino: DestinoDoLink::Pagina(1, 20.0) },
        ];
        let marcadores = [MarcadorDoTitulo { nivel: 1, texto: "1 Introdução".into(), pagina: 2, y: 30.0 }];
        let opcoes = OpcoesPdf { paginas: "1, 3".into(), marca: "CONFIDENCIAL".into(), titulo: "Relatório".into(), ..OpcoesPdf::default() };
        let pdf = gerar_com(&q, &opcoes, &links, &marcadores).unwrap();
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.contains("/Count 2"));
        let fontes: Vec<&str> = s.match_indices("/BaseFont /").map(|(i, _)| s[i + 11..].split([' ', '\n']).next().unwrap_or("")).collect();
        assert!(s.contains("/Subtype /Type0") && fontes.contains(&"LiberationSerif-Bold") && !fontes.contains(&"Times-Bold"), "{fontes:?}");
        // O link para a página 2, que ficou de fora, não entra.
        assert_eq!(s.matches("/Subtype /Link").count(), 2);
        assert!(s.contains("/Type /Outlines") && s.contains("/Title <FEFF"));
        assert!(s.contains("/Creator <FEFF"));
        assert!(gerar_com(&q, &OpcoesPdf { paginas: "2-9".into(), ..OpcoesPdf::default() }, &[], &[]).is_err());
    }

    #[test]
    fn intervalo_de_paginas() {
        assert_eq!(paginas_do_intervalo("", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(paginas_do_intervalo("1-2, 4", 5).unwrap(), vec![0, 1, 3]);
        assert_eq!(paginas_do_intervalo("3-", 5).unwrap(), vec![2, 3, 4]);
        assert!(paginas_do_intervalo("0", 5).is_err() && paginas_do_intervalo("4-2", 5).is_err() && paginas_do_intervalo("x", 5).is_err());
    }
}
