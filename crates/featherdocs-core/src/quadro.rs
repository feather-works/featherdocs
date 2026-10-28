//! O quadro de uma página: tudo que a interface precisa para pintá-la, já
//! pronto e em binário (little-endian), e o teste de acerto do clique.
//!
//! ```text
//! 0                                   nada mudou (a versão que a interface tem ainda vale)
//! 1 | u64 versão | f32 largura | f32 altura | u32 n | n operações
//!
//! operação:  1 Texto      f32 x, f32 y(base), f32 tamanho, u8 estilo, u32 cor, u16 n, n bytes UTF-8
//!            2 Retângulo  f32 x, y, w, h, u32 cor
//!            3 Traço      f32 x1, y1, x2, y2, espessura, u32 cor
//!            4 Imagem     f32 x, y, w, h, u32 n, n bytes UTF-8 (a origem: a data: URL inteira)
//!            5 Girado     f32 x, y (centro), f32 tamanho, f32 ângulo (graus, anti-horário), u8 estilo, u32 cor, u16 n, n bytes
//! ```
//!
//! Coordenadas em pontos, do canto superior esquerdo da página. O cabeçalho e o rodapé
//! (com os campos resolvidos), o plano de fundo, as marcas de formatação e os pontilhados
//! do sumário saem daqui, calculados em Rust.

use crate::layout::{Deco, Linha, COR_CINZA, COR_TEXTO, CORPO};
use crate::medida::largura;
use crate::paginacao::Fatia;
use crate::modelo::{Conteudo, NumeracaoDeLinhas};
use crate::sessao::Sessao;

pub const OP_TEXTO: u8 = 1;
pub const OP_RETANGULO: u8 = 2;
pub const OP_TRACO: u8 = 3;
pub const OP_IMAGEM: u8 = 4;
pub const OP_GIRADO: u8 = 5;

/// A cor das marcas de formatação (¶ e ·).
const COR_MARCA: u32 = 0x7a8f_b8ff;

#[derive(Default)]
struct Escritor {
    bytes: Vec<u8>,
    ops: u32,
    /// O papel (PDF, impressão): sem o que é só da tela, como as guias das tabelas sem borda.
    impresso: bool,
}

impl Escritor {
    fn f(&mut self, v: f32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn texto(&mut self, x: f32, y: f32, tamanho: f32, estilo: u8, cor: u32, t: &str) {
        let t = &t.as_bytes()[..t.len().min(usize::from(u16::MAX))];
        self.bytes.push(OP_TEXTO);
        for v in [x, y, tamanho] {
            self.f(v);
        }
        self.bytes.push(estilo);
        self.bytes.extend_from_slice(&cor.to_le_bytes());
        self.bytes.extend_from_slice(&(t.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(t);
        self.ops += 1;
    }
    fn retangulo(&mut self, x: f32, y: f32, w: f32, h: f32, cor: u32) {
        self.bytes.push(OP_RETANGULO);
        for v in [x, y, w, h] {
            self.f(v);
        }
        self.bytes.extend_from_slice(&cor.to_le_bytes());
        self.ops += 1;
    }
    fn traco(&mut self, p: [f32; 5], cor: u32) {
        self.bytes.push(OP_TRACO);
        for v in p {
            self.f(v);
        }
        self.bytes.extend_from_slice(&cor.to_le_bytes());
        self.ops += 1;
    }
    /// `centro` = (x, y) do meio do texto; `medida` = (tamanho, ângulo em graus).
    fn girado(&mut self, centro: [f32; 2], medida: [f32; 2], estilo: u8, cor: u32, t: &str) {
        let ([x, y], [tamanho, angulo]) = (centro, medida);
        let t = &t.as_bytes()[..t.len().min(usize::from(u16::MAX))];
        self.bytes.push(OP_GIRADO);
        for v in [x, y, tamanho, angulo] {
            self.f(v);
        }
        self.bytes.push(estilo);
        self.bytes.extend_from_slice(&cor.to_le_bytes());
        self.bytes.extend_from_slice(&(t.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(t);
        self.ops += 1;
    }
    fn imagem(&mut self, r: [f32; 4], origem: &str) {
        // u32: a data: URL de uma foto passa fácil dos 64 KiB do u16 (e cortada não abre).
        let o = origem.as_bytes();
        self.bytes.push(OP_IMAGEM);
        for v in r {
            self.f(v);
        }
        self.bytes.extend_from_slice(&(o.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(o);
        self.ops += 1;
    }
}

/// A guia tracejada de uma divisa de célula sem borda: traços de 2 pt com 2 pt de folga.
fn guia(e: &mut Escritor, [x1, y1, x2, y2]: [f32; 4]) {
    const COR: u32 = 0x9fb4_d9ff;
    let (dx, dy) = (x2 - x1, y2 - y1);
    let total = dx.hypot(dy);
    let mut t = 0.0;
    while t < total {
        let (a, b) = (t / total, ((t + 2.0) / total).min(1.0));
        e.traco([x1 + dx * a, y1 + dy * a, x1 + dx * b, y1 + dy * b, 0.5], COR);
        t += 4.0;
    }
}

impl Sessao {
    /// O quadro da `pagina` (0, 1…); `[0]` se a interface já tem esta `versao`.
    pub fn quadro(&self, pagina: usize, versao_conhecida: u64) -> Result<Vec<u8>, String> {
        if versao_conhecida == self.versao {
            return Ok(vec![0]);
        }
        self.montar(pagina, false)
    }

    /// O quadro da `pagina` como sai no papel: sem a grade e sem as marcas de quebra.
    pub fn quadro_impresso(&self, pagina: usize) -> Result<Vec<u8>, String> {
        self.montar(pagina, true)
    }

    fn montar(&self, pagina: usize, impresso: bool) -> Result<Vec<u8>, String> {
        let p = self.paginas.get(pagina).ok_or_else(|| format!("A página {} não existe (há {}).", pagina + 1, self.paginas.len()))?;
        let mut e = Escritor { impresso, ..Escritor::default() };
        self.plano_de_fundo(&mut e);
        self.decoracoes(&mut e, pagina, true);
        if self.grade && !impresso {
            self.linhas_de_grade(&mut e);
        }
        let mut linha = if self.doc.extras.numerar_linhas == NumeracaoDeLinhas::Continua { self.linhas_antes(pagina) } else { 0 };
        for f in p.fatias.iter().filter(|f| !(impresso && self.layouts[f.bloco].so_na_tela)) {
            self.fatia(&mut e, f, &mut linha);
        }
        if !impresso {
            self.quebra_antes_da_proxima(&mut e, pagina);
        }
        self.notas_da_pagina(&mut e, pagina);
        self.cabecalho_e_rodape(&mut e, pagina);
        self.decoracoes(&mut e, pagina, false);
        let cfg = self.doc.pagina;
        let mut saida = vec![1u8];
        saida.extend_from_slice(&self.versao.to_le_bytes());
        saida.extend_from_slice(&cfg.largura.to_le_bytes());
        saida.extend_from_slice(&cfg.altura.to_le_bytes());
        saida.extend_from_slice(&e.ops.to_le_bytes());
        saida.extend_from_slice(&e.bytes);
        Ok(saida)
    }

    /// A página seguinte começa com um parágrafo de quebra de página antes (`w:pageBreakBefore`): a
    /// marca "quebra de página" sai no fim desta, como a do bloco de quebra (só na tela).
    fn quebra_antes_da_proxima(&self, e: &mut Escritor, pagina: usize) {
        let Some(f) = self.paginas.get(pagina + 1).and_then(|p| p.fatias.first()) else { return };
        if f.de != 0 || self.doc.blocos[f.bloco].atrib.mais.fluxo & crate::modelo::QUEBRA_ANTES == 0 {
            return;
        }
        let cfg = self.doc.pagina;
        let y = self.paginas[pagina].fatias.last().map_or(0.0, |u| u.y + u.altura);
        let marca = crate::layout::marca_de_quebra("quebra de página", cfg.largura_util(), true);
        self.linha(e, &marca.linhas[0], cfg.margem_esq, cfg.margem_sup + y + 2.0);
    }

    /// Quantas linhas numeráveis (de texto corrido) vêm antes da página `pagina`.
    pub(crate) fn linhas_antes(&self, pagina: usize) -> usize {
        self.paginas[..pagina].iter().flat_map(|p| &p.fatias).filter(|f| crate::digitacao::tem_texto_corrido(&self.doc.blocos[f.bloco].conteudo)).map(|f| f.ate - f.de).sum()
    }

    /// Uma grade de 0,5 cm sobre a área do texto, bem clara (Exibir → Linhas de grade).
    fn linhas_de_grade(&self, e: &mut Escritor) {
        const PASSO: f32 = 72.0 / 2.54 / 2.0;
        const COR: u32 = 0xc8d2_e6aa;
        let cfg = self.doc.pagina;
        let (x1, y1, x2, y2) = (cfg.margem_esq, cfg.margem_sup, cfg.largura - cfg.margem_dir, cfg.altura - cfg.margem_inf);
        let mut y = y1;
        while y <= y2 + 0.01 {
            e.traco([x1, y, x2, y, 0.4], COR);
            y += PASSO;
        }
        let mut x = x1;
        while x <= x2 + 0.01 {
            e.traco([x, y1, x, y2, 0.4], COR);
            x += PASSO;
        }
    }

    fn fatia(&self, e: &mut Escritor, f: &Fatia, numero: &mut usize) {
        let cfg = self.doc.pagina;
        let lay = &self.layouts[f.bloco];
        let mut y = cfg.margem_sup + f.y;
        let texto = crate::digitacao::tem_texto_corrido(&self.doc.blocos[f.bloco].conteudo);
        let numerar = texto && self.doc.extras.numerar_linhas != NumeracaoDeLinhas::Nenhuma;
        for (k, linha) in lay.linhas[f.de..f.ate].iter().enumerate() {
            self.linha(e, linha, cfg.margem_esq, y);
            if !linha.comentarios.is_empty() {
                self.baloes(e, linha, y);
            }
            if numerar {
                // O número fica na margem esquerda, alinhado à direita a 0,5 cm do texto, na linha de base.
                *numero += 1;
                let n = numero.to_string();
                let base = linha.pedacos.first().map_or(linha.base, |p| p.y);
                e.texto(cfg.margem_esq - 14.0 - largura(&n, 0, 8.0), y + base, 8.0, 0, COR_CINZA, &n);
            }
            if self.marcas && texto && f.de + k + 1 == lay.linhas.len() {
                pilcrow(e, linha, cfg.margem_esq, y);
            }
            if let (Some(n), true) = (&lay.numero, k == 0 && f.de == 0) {
                let base = linha.pedacos.first().map_or(linha.base, |p| p.y);
                e.texto(cfg.margem_esq + n.x, y + base, n.tamanho, n.estilo, n.cor, &n.texto);
            }
            if let (Some(m), Some(x)) = (&f.marcador, lay.marcador_x) {
                if k == 0 {
                    let base = linha.pedacos.first().map_or(linha.altura * 0.7, |p| p.y);
                    let tam = if linha.tam > 0.0 { linha.tam } else { CORPO };
                    let cor = if lay.cor_marcador != 0 { lay.cor_marcador } else { COR_TEXTO };
                    // O marcador na família do texto do item (com serifa no LaTeX).
                    let familia = linha.pedacos.first().map_or(0, |p| p.estilo & crate::modelo::SERIFA);
                    if m == crate::layout::BOLINHA {
                        // O `\bigcirc`: 0,8 em de diâmetro, centrado um pouco acima da linha de base.
                        let (raio, cx, cy) = (0.4 * tam, cfg.margem_esq + x + 0.4 * tam, y + base - 0.3 * tam);
                        let ponto = |i: u8| {
                            let t = f32::from(i) * std::f32::consts::TAU / 24.0;
                            (cx + raio * t.cos(), cy + raio * t.sin())
                        };
                        for i in 0..24 {
                            let ((xa, ya), (xb, yb)) = (ponto(i), ponto(i + 1));
                            e.traco([xa, ya, xb, yb, 0.5], cor);
                        }
                    } else {
                        e.texto(cfg.margem_esq + x, y + base, tam, familia | lay.estilo_marcador, cor, m);
                    }
                }
            }
            y += linha.altura;
        }
    }

    fn linha(&self, e: &mut Escritor, l: &Linha, x0: f32, y0: f32) {
        for d in &l.deco {
            match d {
                Deco::Retangulo { x, y, w, h, cor } => e.retangulo(x0 + x, y0 + y, *w, *h, *cor),
                Deco::Traco { x1, y1, x2, y2, espessura, cor } => e.traco([x0 + x1, y0 + y1, x0 + x2, y0 + y2, *espessura], *cor),
                Deco::Imagem { x, y, w, h, origem } => e.imagem([x0 + x, y0 + y, *w, *h], origem),
                Deco::Texto { x, y, texto, tamanho, estilo, cor } => e.texto(x0 + x, y0 + y, *tamanho, *estilo, *cor, texto),
                Deco::Guia { x1, y1, x2, y2 } if !e.impresso => guia(e, [x0 + x1, y0 + y1, x0 + x2, y0 + y2]),
                Deco::Guia { .. } => {}
            }
        }
        for p in l.pedacos.iter().filter(|p| p.realce != 0) {
            let w = largura(&p.texto, p.estilo, p.tamanho);
            e.retangulo(x0 + p.x, y0 + p.y - p.tamanho * 0.95 - p.desloc, w, p.tamanho * 1.2, p.realce);
        }
        for p in &l.pedacos {
            e.texto(x0 + p.x, y0 + p.y - p.desloc, p.tamanho, p.estilo, p.cor, &p.texto);
            if self.marcas && p.texto.contains(' ') && p.estilo & crate::modelo::MATEMATICA == 0 {
                pontos_nos_espacos(e, p, x0, y0);
            }
        }
        if let (Some(alvo), true) = (l.alvo, l.sumario & 1 != 0) {
            self.numero_do_sumario(e, l, alvo, x0, y0);
        }
    }

    /// Um balão na margem direita para cada comentário que começa na linha (aberto em amarelo,
    /// resolvido em cinza).
    fn baloes(&self, e: &mut Escritor, l: &Linha, y: f32) {
        let x = self.doc.pagina.largura - self.doc.pagina.margem_dir + 10.0;
        for (k, id) in l.comentarios.iter().enumerate() {
            let resolvido = self.doc.comentarios.iter().find(|c| c.id == *id).is_some_and(|c| c.resolvido);
            let cor = if resolvido { 0xdadc_e0ff } else { 0xfbbc_04ff };
            let (bx, by) = (x + k as f32 * 16.0, y + 1.0);
            e.retangulo(bx, by, 12.0, 9.0, cor);
            e.traco([bx + 2.0, by + 9.0, bx + 2.0, by + 12.0, 1.5], cor);
        }
    }

    /// O traço e as notas de rodapé no pé da página; o título e as notas de fim onde a paginação as pôs.
    fn notas_da_pagina(&self, e: &mut Escritor, pagina: usize) {
        let cfg = self.doc.pagina;
        if let Some(y) = self.topo_das_notas(pagina) {
            e.traco([cfg.margem_esq, y, cfg.margem_esq + cfg.largura_util() / 3.0, y, 0.5], COR_TEXTO);
        }
        for c in self.caixas_das_notas(pagina) {
            let Some(n) = self.nota_composta(c.nota) else { continue };
            let mut y = c.y;
            for l in &n.linhas {
                self.linha(e, l, cfg.margem_esq, y);
                y += l.altura;
            }
        }
        if let Some((topo, _)) = self.paginas[pagina].notas_fim.iter().find(|(_, r)| r.is_none()) {
            let y = cfg.margem_sup + topo + crate::objetos::TITULO_DAS_NOTAS * 0.6;
            e.texto(cfg.margem_esq, y, 15.0, crate::modelo::NEGRITO, COR_TEXTO, self.titulo_das_notas());
        }
    }

    /// Pontilhado e número da página, alinhado à direita da área útil.
    fn numero_do_sumario(&self, e: &mut Escritor, l: &Linha, alvo: u64, x0: f32, y0: f32) {
        let (Some(p), Some(ultimo)) = (l.pedacos.first(), l.pedacos.last()) else { return };
        let numero = self.pagina_do_bloco(alvo).map_or("?".to_string(), |n| self.numero_da_pagina(n).to_string());
        let direita = x0 + self.doc.pagina.largura_util();
        let w_num = largura(&numero, p.estilo, p.tamanho);
        let fim_texto = x0 + ultimo.x + largura(&ultimo.texto, ultimo.estilo, ultimo.tamanho);
        let disponivel = direita - w_num - fim_texto - 8.0;
        // No jeito do LaTeX, os pontos são da fonte e da cor do texto (sem negrito).
        let (estilo_ponto, cor_ponto) = if l.sumario & 4 != 0 { (p.estilo & crate::modelo::SERIFA, p.cor) } else { (0, COR_CINZA) };
        let w_ponto = largura(". ", estilo_ponto, p.tamanho);
        if l.sumario & 2 != 0 && disponivel > w_ponto {
            let pontos = ". ".repeat((disponivel / w_ponto) as usize);
            e.texto(fim_texto + 4.0, y0 + p.y, p.tamanho, estilo_ponto, cor_ponto, &pontos);
        }
        e.texto(direita - w_num, y0 + p.y, p.tamanho, p.estilo, p.cor, &numero);
    }

    /// Cor do papel, marca d'água na diagonal e borda da página (antes do texto, que fica por cima).
    /// As decorações desta página que ficam atrás (`atras`) ou na frente do texto: as sem âncora em
    /// todas as páginas; as ancoradas, na página em que o bloco delas começa.
    fn decoracoes(&self, e: &mut Escritor, pagina: usize, atras: bool) {
        for d in self.doc.extras.decoracoes.iter().filter(|d| d.atras == atras) {
            let aqui = match d.bloco {
                None => true,
                Some(id) => self.doc.blocos.iter().position(|b| b.id == id)
                    .is_some_and(|i| self.paginas.get(pagina).is_some_and(|p| p.fatias.iter().any(|f| f.bloco == i && f.de == 0))),
            };
            if aqui && d.largura > 0.0 && d.altura > 0.0 {
                e.imagem([d.x, d.y, d.largura, d.altura], &d.origem);
            }
        }
    }

    fn plano_de_fundo(&self, e: &mut Escritor) {
        let (cfg, x) = (self.doc.pagina, &self.doc.extras);
        if x.cor_pagina != 0 {
            e.retangulo(0.0, 0.0, cfg.largura, cfg.altura, x.cor_pagina);
        }
        if !x.marca.is_empty() {
            // O texto ocupa uns 70% da diagonal; a cor padrão é um cinza bem claro.
            let diagonal = cfg.largura.hypot(cfg.altura);
            let angulo = (cfg.altura / cfg.largura).atan().to_degrees();
            let w1000 = largura(&x.marca, crate::modelo::NEGRITO, 1000.0) / 1000.0;
            let tamanho = (diagonal * 0.7 / w1000.max(0.1)).min(cfg.altura * 0.25);
            let cor = if x.marca_cor != 0 { x.marca_cor } else { 0xc0c0_c066 };
            e.girado([cfg.largura / 2.0, cfg.altura / 2.0], [tamanho, angulo], crate::modelo::NEGRITO, cor, &x.marca);
        }
        if x.borda_cor != 0 {
            let d = x.borda_distancia.max(0.0);
            let (x1, y1, x2, y2) = (d, d, cfg.largura - d, cfg.altura - d);
            let w = x.borda_espessura.max(0.25);
            for p in [[x1, y1, x2, y1, w], [x2, y1, x2, y2, w], [x2, y2, x1, y2, w], [x1, y2, x1, y1, w]] {
                e.traco(p, x.borda_cor);
            }
        }
    }

    /// Cabeçalho e rodapé: três lugares (esquerda, centro, direita) com os campos resolvidos.
    fn cabecalho_e_rodape(&self, e: &mut Escritor, pagina: usize) {
        let x = &self.doc.extras;
        let par = x.documento.pares_diferentes && self.numero_da_pagina(pagina) % 2 == 0;
        let (cabecalho, rodape) = if x.primeira_diferente && pagina == 0 {
            (&x.cabecalho_primeira, &x.rodape_primeira)
        } else if par {
            (&x.documento.cabecalho_par, &x.documento.rodape_par)
        } else {
            (&x.cabecalho, &x.rodape)
        };
        let cfg = self.doc.pagina;
        // A linha de base: na metade da margem ou, com a distância do Word, logo abaixo dela.
        let corpo = |f: &crate::modelo::Faixas| if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
        let m = &x.documento;
        let y_cab = if m.distancia_cabecalho > 0.0 { m.distancia_cabecalho + corpo(cabecalho) * 0.9 } else { cfg.margem_sup / 2.0 + 3.0 };
        let y_rod = if m.distancia_rodape > 0.0 { cfg.altura - m.distancia_rodape - corpo(rodape) * 0.25 } else { cfg.altura - cfg.margem_inf / 2.0 + 3.0 };
        self.faixas(e, cabecalho, y_cab, pagina, false);
        self.faixas(e, rodape, y_rod, pagina, true);
    }

    fn faixas(&self, e: &mut Escritor, f: &crate::modelo::Faixas, y: f32, pagina: usize, rodape: bool) {
        let cfg = self.doc.pagina;
        let tam = if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
        let cor = if f.cor != 0 { f.cor } else { COR_CINZA };
        // A imagem fica no meio; no cabeçalho, com a base a 25 pt do texto (o `headsep` do LaTeX).
        if !f.imagem.is_empty() && f.imagem_largura > 0.0 && f.imagem_altura > 0.0 {
            let (w, h) = (f.imagem_largura, f.imagem_altura);
            let topo = if rodape { cfg.altura - cfg.margem_inf + 12.0 } else { (cfg.margem_sup - 25.0 - h).max(0.0) };
            e.imagem([(cfg.largura - w) / 2.0, topo, w, h], &f.imagem);
        }
        for (texto, onde) in [(&f.esquerda, 0), (&f.centro, 1), (&f.direita, 2)] {
            if texto.is_empty() {
                continue;
            }
            let t = self.resolver_campos(texto, pagina);
            let w = largura(&t, f.estilo, tam);
            let x = match onde {
                0 => cfg.margem_esq,
                1 => (cfg.largura - w) / 2.0,
                _ => cfg.largura - cfg.margem_dir - w,
            };
            e.texto(x, y, tam, f.estilo, cor, &t);
        }
        // O fio do `fancyhdr`: logo abaixo da linha do cabeçalho, logo acima da do rodapé, na largura do texto.
        if f.fio > 0.0 {
            let fy = if rodape { y - tam - 2.0 } else { y + 0.3 * tam + 1.0 };
            e.traco([cfg.margem_esq, fy, cfg.largura - cfg.margem_dir, fy, f.fio], if f.cor != 0 { f.cor } else { COR_TEXTO });
        }
    }

    /// Troca `{pagina}`, `{total}`, `{pagina_romano}`, `{titulo}` e `{autor}` pelos valores desta página.
    pub fn resolver_campos(&self, texto: &str, pagina: usize) -> String {
        texto
            .replace("{pagina_romano}", &crate::paginacao::romano(self.numero_da_pagina(pagina) as u32).to_lowercase())
            .replace("{pagina}", &crate::modelo::numero_de_pagina(self.numero_da_pagina(pagina), &self.doc.extras.documento.formato_pagina))
            .replace("{total}", &crate::modelo::numero_de_pagina(self.numero_da_pagina(self.paginas.len().saturating_sub(1)), &self.doc.extras.documento.formato_pagina))
            .replace("{titulo}", &self.doc.meta.titulo)
            .replace("{autor}", &self.doc.meta.autor)
    }
}

/// O ¶ no fim do parágrafo (marcas de formatação).
/// Para onde leva uma área clicável do PDF.
#[derive(Clone, Debug, PartialEq)]
pub enum DestinoDoLink {
    Url(String),
    /// Uma página (contada do 0) e a altura a partir do topo dela.
    Pagina(usize, f32),
}

/// Uma área clicável: a página, (x, y, largura, altura) a partir do topo, e o destino.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkDaPagina {
    pub pagina: usize,
    pub area: [f32; 4],
    pub destino: DestinoDoLink,
}

/// Um título para a árvore de marcadores do PDF: nível, texto (com o número) e onde fica.
#[derive(Clone, Debug, PartialEq)]
pub struct MarcadorDoTitulo {
    pub nivel: u8,
    pub texto: String,
    pub pagina: usize,
    pub y: f32,
}

impl Sessao {
    /// A página e a altura (do topo) em que o bloco começa.
    fn onde_comeca(&self) -> std::collections::HashMap<usize, (usize, f32)> {
        let mut onde = std::collections::HashMap::new();
        for (n, p) in self.paginas.iter().enumerate() {
            for f in &p.fatias {
                onde.entry(f.bloco).or_insert((n, self.doc.pagina.margem_sup + f.y));
            }
        }
        onde
    }

    /// Os links do documento já postos na página (texto com link e linhas do sumário), para o PDF.
    pub fn links_do_pdf(&self) -> Vec<LinkDaPagina> {
        let onde = self.onde_comeca();
        let interno = |link: &str| crate::referencias::destino_do_link(link).and_then(|id| self.idx(id)).and_then(|i| onde.get(&i)).map(|&(p, y)| DestinoDoLink::Pagina(p, y));
        let cfg = self.doc.pagina;
        let mut saida = Vec::new();
        for (n, p) in self.paginas.iter().enumerate() {
            for f in &p.fatias {
                let lay = &self.layouts[f.bloco];
                let trechos = match &self.doc.blocos[f.bloco].conteudo {
                    Conteudo::Paragrafo { texto } | Conteudo::Titulo { texto, .. } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => Some(texto),
                    _ => None,
                };
                let mut y = cfg.margem_sup + f.y;
                for l in &lay.linhas[f.de..f.ate] {
                    if let (Some(alvo), true) = (l.alvo, l.sumario & 1 != 0) {
                        if let Some(&(pg, ya)) = self.idx(alvo).and_then(|i| onde.get(&i)) {
                            saida.push(LinkDaPagina { pagina: n, area: [cfg.margem_esq, y, cfg.largura_util(), l.altura], destino: DestinoDoLink::Pagina(pg, ya) });
                        }
                    }
                    for pd in l.pedacos.iter().filter(|pd| pd.celula.is_none()) {
                        let Some(link) = trechos.and_then(|t| link_no_caractere(t, pd.de)) else { continue };
                        let destino = if link.starts_with('#') { interno(&link) } else { Some(DestinoDoLink::Url(link)) };
                        if let Some(destino) = destino {
                            let w = largura(&pd.texto, pd.estilo, pd.tamanho);
                            saida.push(LinkDaPagina { pagina: n, area: [cfg.margem_esq + pd.x, y + pd.y - pd.tamanho * 0.95 - pd.desloc, w, pd.tamanho * 1.2], destino });
                        }
                    }
                    y += l.altura;
                }
            }
        }
        saida
    }

    /// Os títulos que entram no sumário, com o número e o lugar, para os marcadores do PDF.
    pub fn marcadores_do_pdf(&self) -> Vec<MarcadorDoTitulo> {
        let onde = self.onde_comeca();
        self.entradas_do_sumario()
            .into_iter()
            .filter_map(|e| {
                let &(pagina, y) = onde.get(&self.idx(e.id)?)?;
                Some(MarcadorDoTitulo { nivel: e.nivel, texto: e.texto.split_whitespace().collect::<Vec<_>>().join(" "), pagina, y })
            })
            .collect()
    }
}

/// O link do trecho em que cai o caractere `de` (contado em caracteres).
fn link_no_caractere(t: &[crate::modelo::Trecho], de: usize) -> Option<String> {
    let mut n = 0;
    for tr in t {
        let k = tr.texto.chars().count();
        if de < n + k {
            return tr.link.clone();
        }
        n += k;
    }
    None
}

fn pilcrow(e: &mut Escritor, l: &Linha, x0: f32, y0: f32) {
    let (x, base, tam) = match l.pedacos.last() {
        Some(p) => (p.x + largura(&p.texto, p.estilo, p.tamanho), p.y, p.tamanho),
        None => (l.x0, l.base, l.tam),
    };
    e.texto(x0 + x + 1.0, y0 + base, tam, 0, COR_MARCA, "¶");
}

/// Um ponto no meio de cada espaço do pedaço (marcas de formatação).
fn pontos_nos_espacos(e: &mut Escritor, p: &crate::layout::Pedaco, x0: f32, y0: f32) {
    let mut x = p.x;
    for c in p.texto.chars() {
        let w = f32::from(crate::medida::largura_do_caractere(c, p.estilo)) * p.tamanho / 1000.0;
        if c == ' ' {
            let wp = largura("·", 0, p.tamanho);
            e.texto(x0 + x + (w - wp) / 2.0, y0 + p.y - p.desloc, p.tamanho, 0, COR_MARCA, "·");
        }
        x += w;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn versao_conhecida_devolve_um_byte() {
        let s = Sessao::default();
        assert_eq!(s.quadro(0, s.versao()).unwrap(), vec![0]);
    }

    #[test]
    fn links_e_marcadores_para_o_pdf() {
        use crate::modelo::{Documento, Trecho};
        let mut d = Documento::default();
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: crate::inline::analisar("Introdução") });
        let alvo = d.blocos[0].id;
        let t = |texto: &str, link: Option<String>| Trecho { texto: texto.into(), link, ..Trecho::default() };
        d.acrescentar(Conteudo::Paragrafo { texto: vec![t("Veja ", None), t("o site", Some("https://exemplo.org".into())), t(" e a ", None), t("seção", Some(format!("#bloco:{alvo}")))] });
        let s = Sessao::com_documento(d);
        let links = s.links_do_pdf();
        assert_eq!(links.len(), 2, "{links:?}");
        assert_eq!(links[0].destino, DestinoDoLink::Url("https://exemplo.org".into()));
        assert!(matches!(links[1].destino, DestinoDoLink::Pagina(0, _)));
        assert!(links[0].area[2] > 10.0 && links[1].area[0] > links[0].area[0]);
        let m = s.marcadores_do_pdf();
        assert_eq!((m.len(), m[0].nivel, m[0].pagina), (1, 1, 0));
        assert!(m[0].texto.ends_with("Introdução"));
    }

    #[test]
    fn quadro_tem_cabecalho_e_numero_de_pagina() {
        let s = Sessao::default();
        let q = s.quadro(0, 0).unwrap();
        assert_eq!(q[0], 1);
        let n = u32::from_le_bytes(q[17..21].try_into().unwrap());
        assert!(n >= 1, "ao menos o número da página");
    }

    #[test]
    fn tabela_do_latex_que_cabe_numa_pagina_nao_se_parte() {
        use crate::modelo::{Celula, Documento};
        // Com 20 a 60 parágrafos antes, a tabela cai em algum momento no pé da página.
        for n in 20..60 {
            let mut d = Documento::default();
            for _ in 0..n {
                d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar("Um parágrafo de enchimento para chegar perto do pé da página.") });
            }
            let linhas: Vec<Vec<Celula>> = (0..8).map(|i| vec![crate::inline::analisar(&format!("linha {i}"))]).collect();
            d.acrescentar(Conteudo::Tabela { cabecalho: true, linhas });
            let ultimo = d.blocos.len() - 1;
            d.blocos[ultimo].latex = "l\n".into();
            let s = Sessao::com_documento(d);
            let paginas: Vec<usize> = s.paginas.iter().enumerate().filter(|(_, p)| p.fatias.iter().any(|f| f.bloco == ultimo)).map(|(i, _)| i).collect();
            assert_eq!(paginas.len(), 1, "com {n} parágrafos a tabela ficou em {paginas:?}");
        }
    }

    #[test]
    fn tabela_sem_borda_tem_guia_so_na_tela() {
        use crate::modelo::{BordasDeTabela, Documento, FioDeBorda};
        let mut d = Documento::default();
        let cel = |t: &str| crate::inline::analisar(t);
        let id = d.acrescentar(Conteudo::Tabela { cabecalho: false, linhas: vec![vec![cel("a"), cel("b")]] });
        let nil = || Some(FioDeBorda { estilo: "nil".into(), espessura: 0, cor: 0 });
        let b = d.blocos.iter_mut().find(|b| b.id == id).unwrap();
        b.tabela.bordas = BordasDeTabela { cima: nil(), esquerda: nil(), baixo: nil(), direita: nil(), dentro_h: nil(), dentro_v: nil() };
        let s = Sessao::com_documento(d);
        let cor = 0x9fb4_d9ffu32.to_le_bytes();
        let tem = |q: Vec<u8>| q.windows(4).any(|w| w == cor);
        assert!(tem(s.quadro(0, 0).unwrap()), "a tela mostra a guia");
        assert!(!tem(s.quadro_impresso(0).unwrap()), "o papel não");
    }

    #[test]
    fn marca_de_quebra_so_na_tela() {
        use crate::modelo::Documento;
        let mut d = Documento::default();
        d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar("antes") });
        d.acrescentar(Conteudo::QuebraDePagina);
        d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar("depois") });
        let s = Sessao::com_documento(d);
        let tem = |q: Vec<u8>| q.windows(16).any(|w| w == "quebra de página".as_bytes()[..16].as_ref());
        assert!(tem(s.quadro(0, 0).unwrap()), "a tela mostra a marca");
        assert!(!tem(s.quadro_impresso(0).unwrap()), "o papel não");
    }

    #[test]
    fn quebra_antes_do_paragrafo_mostra_a_marca_so_na_tela() {
        use crate::modelo::{Documento, QUEBRA_ANTES};
        let mut d = Documento::default();
        d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar("antes") });
        d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar("depois") });
        let ultimo = d.blocos.len() - 1;
        d.blocos[ultimo].atrib.mais.fluxo |= QUEBRA_ANTES;
        let s = Sessao::com_documento(d);
        assert!(s.paginas.len() >= 2, "o parágrafo começa a página seguinte");
        let tem = |q: Vec<u8>| q.windows(16).any(|w| w == "quebra de página".as_bytes()[..16].as_ref());
        assert!(tem(s.quadro(0, 0).unwrap()), "a tela mostra a marca");
        assert!(!tem(s.quadro_impresso(0).unwrap()), "o papel não");
    }

    #[test]
    fn nota_de_rodape_sai_no_pe_com_o_traco() {
        let mut s = Sessao::default();
        s.nota_inserir(false, "texto da nota").unwrap();
        let q = s.quadro(0, 0).unwrap();
        let achou = q.windows(13).any(|w| w == b"texto da nota");
        assert!(achou, "o texto da nota está no quadro");
    }

    #[test]
    fn pagina_inexistente_e_erro_em_portugues() {
        assert!(Sessao::default().quadro(7, 0).unwrap_err().contains("não existe"));
    }
}
