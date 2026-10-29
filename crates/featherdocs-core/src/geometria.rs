//! Onde as coisas estão na página: a posição do cursor, o que um clique acerta, os
//! retângulos da seleção e o movimento vertical. Tudo em pontos, do canto superior
//! esquerdo da página. Usa a composição guardada (`LayoutBloco`) e as larguras de
//! `medida`, as mesmas que a quebra de linha usou — por isso o cursor cai exatamente
//! entre as letras que a interface desenhou.

use serde::Serialize;

use crate::alvo::{eh_atomico, tamanho_alvo, Cel};
use crate::cursor::Pos;
use crate::layout::{Linha, Pedaco};
use crate::medida::{largura, largura_do_caractere};
use crate::modelo::Conteudo;
use crate::paginacao::Fatia;
use crate::sessao::Sessao;

/// A posição do cursor: o topo da barrinha e a altura dela.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Geo {
    pub pagina: usize,
    pub x: f32,
    pub y: f32,
    pub h: f32,
}

/// Um retângulo de seleção numa página.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Ret {
    pub pagina: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Tudo que a interface precisa para desenhar o cursor depois de uma ação: um comando só.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Visao {
    pub cursor: Option<Geo>,
    pub retangulos: Vec<Ret>,
    /// Os achados da busca à vista (o realce).
    pub achados: Vec<Ret>,
    pub formato: crate::formato::Formato,
}

/// O que a seleção cobre num bloco: os caracteres `[a, b)` de um texto de `total` caracteres.
#[derive(Clone, Copy)]
struct Selecionado {
    a: usize,
    b: usize,
    total: usize,
    /// A seleção continua depois do fim deste parágrafo (mostra o "enter" selecionado).
    continua: bool,
    celula: Cel,
}

/// A parte de cima do corpo do texto acima da linha de base, e a altura do cursor, em múltiplos do corpo.
const SUBIDA: f32 = 0.84;
const ALTURA_DO_CURSOR: f32 = 1.2;

/// Onde, dentro de uma linha, está o caractere `desl`: `(x, linha de base, corpo)`.
fn x_na_linha(pedacos: &[&Pedaco], desl: usize, padrao: (f32, f32, f32)) -> (f32, f32, f32) {
    let Some(ultimo) = pedacos.last() else { return padrao };
    for (k, p) in pedacos.iter().enumerate() {
        let n = p.caracteres();
        if desl >= p.de && (desl < p.de + n || k + 1 == pedacos.len()) {
            let dentro = desl.min(p.de + n) - p.de;
            let w = if p.atomo {
                if dentro == 0 { 0.0 } else { largura(&p.texto, p.estilo, p.tamanho) }
            } else if p.estilo & crate::modelo::MATEMATICA != 0 {
                // A fórmula não tem um caractere por glifo: o cursor anda em proporção ao LaTeX.
                largura(&p.texto, p.estilo, p.tamanho) * dentro as f32 / n.max(1) as f32
            } else {
                largura(&p.texto.chars().take(dentro).collect::<String>(), p.estilo, p.tamanho)
            };
            return (p.x + w, p.y, p.tamanho);
        }
        if desl < p.de {
            return (p.x, p.y, p.tamanho);
        }
    }
    (ultimo.x, ultimo.y, ultimo.tamanho)
}

/// O caractere (entre os dos `pedacos`) mais perto da coordenada `x`.
fn desl_no_x(pedacos: &[&Pedaco], x: f32, padrao: usize) -> usize {
    let Some(ultimo) = pedacos.last() else { return padrao };
    for p in pedacos {
        let fim = p.x + largura(&p.texto, p.estilo, p.tamanho);
        if x >= fim {
            continue;
        }
        if x <= p.x {
            return p.de;
        }
        if p.atomo {
            return p.de + usize::from(x >= (p.x + fim) / 2.0);
        }
        if p.estilo & crate::modelo::MATEMATICA != 0 {
            let n = p.caracteres();
            let k = (((x - p.x) / (fim - p.x).max(0.01)) * n as f32).round() as usize;
            return p.de + k.min(n);
        }
        let mut acumulado = p.x;
        for (k, c) in p.texto.chars().enumerate() {
            let w = f32::from(largura_do_caractere(c, p.estilo)) * p.tamanho / 1000.0;
            if x < acumulado + w / 2.0 {
                return p.de + k;
            }
            acumulado += w;
        }
        return p.de + p.caracteres();
    }
    ultimo.de + ultimo.caracteres()
}

/// Entre as linhas de texto de uma célula (agrupadas pela linha de base), as da mais próxima de `y`.
fn linha_da_celula<'a>(pedacos: &[&'a Pedaco], y: f32) -> Vec<&'a Pedaco> {
    let Some(melhor) = pedacos.iter().map(|p| p.y).min_by(|a, b| (a - y).abs().total_cmp(&(b - y).abs())) else { return Vec::new() };
    pedacos.iter().filter(|p| (p.y - melhor).abs() < 0.5).copied().collect()
}

impl Sessao {
    /// As fatias (página, fatia) de um bloco, em ordem.
    pub(crate) fn fatias_do_bloco(&self, i: usize) -> Vec<(usize, &Fatia)> {
        let mut v = Vec::new();
        let mut p = self.primeira_pagina_do_bloco.get(i).copied().unwrap_or(0);
        while let Some(pag) = self.paginas.get(p) {
            let antes = v.len();
            v.extend(pag.fatias.iter().filter(|f| f.bloco == i).map(|f| (p, f)));
            if v.len() == antes {
                break;
            }
            p += 1;
        }
        v
    }

    /// O índice da linha de um bloco que contém o caractere `desl` (a linha onde ele começa).
    pub(crate) fn linha_do_desl(&self, i: usize, desl: usize) -> usize {
        let linhas = &self.layouts[i].linhas;
        (0..linhas.len()).rev().find(|&k| linhas[k].de <= desl).unwrap_or(0)
    }

    fn pedacos_do_alvo(&self, i: usize, li: usize, celula: Cel) -> Vec<&Pedaco> {
        let linha = &self.layouts[i].linhas[li];
        match celula {
            Some((_, c)) => linha.pedacos.iter().filter(|p| p.celula == Some(c as u16)).collect(),
            None => linha.pedacos.iter().collect(),
        }
    }

    /// `(x, linha de base, corpo)` de um ponto, dentro da linha `li` do bloco `i`.
    fn ponto_na_linha(&self, i: usize, li: usize, celula: Cel, desl: usize) -> (f32, f32, f32) {
        let linha = &self.layouts[i].linhas[li];
        let padrao = match celula {
            Some((_, c)) => linha.celulas.get(c as usize).map_or((0.0, linha.base, linha.tam), |x| (x.x0, x.base, x.tam)),
            None => (linha.x0, linha.base, linha.tam),
        };
        x_na_linha(&self.pedacos_do_alvo(i, li, celula), desl, padrao)
    }

    /// A linha do bloco em que o ponto está (para tabela, a linha da tabela).
    fn linha_do_pos(&self, i: usize, p: &Pos) -> usize {
        match p.celula {
            Some((l, _)) => (l as usize).min(self.layouts[i].linhas.len().saturating_sub(1)),
            None => self.linha_do_desl(i, p.desl),
        }
    }

    /// Onde desenhar o cursor.
    pub fn geometria(&self, p: &Pos) -> Option<Geo> {
        let i = self.idx(p.bloco)?;
        let li = self.linha_do_pos(i, p);
        let cfg = self.doc.pagina;
        let (pagina, f) = self.fatias_do_bloco(i).into_iter().find(|(_, f)| li >= f.de && li < f.ate)?;
        let topo = f.y + self.layouts[i].linhas[f.de..li].iter().map(|l| l.altura).sum::<f32>();
        if eh_atomico(&self.doc.blocos[i].conteudo) {
            return Some(Geo { pagina, x: cfg.margem_esq + f.x, y: cfg.margem_sup + topo, h: self.layouts[i].linhas[li].altura.min(24.0) });
        }
        let (x, base, tam) = self.ponto_na_linha(i, li, p.celula, p.desl);
        Some(Geo { pagina, x: cfg.margem_esq + f.x + x, y: cfg.margem_sup + topo + base - tam * SUBIDA, h: tam * ALTURA_DO_CURSOR })
    }

    /// O ponto do documento sob a coordenada (`x`, `y`) da página; o mais perto, se está fora do texto.
    pub fn pos_na_pagina(&self, pagina: usize, x: f32, y: f32) -> Option<Pos> {
        let pag = self.paginas.get(pagina)?;
        let cfg = self.doc.pagina;
        let (x, y) = (x - cfg.margem_esq, y - cfg.margem_sup);
        // Primeiro a coluna (a que está sob o ponto, ou a mais perto), depois a altura.
        let largura = self.doc.colunas().1;
        let fora = |f: &Fatia| if x < f.x { f.x - x } else { (x - (f.x + largura)).max(0.0) };
        let dist = |f: &Fatia| if y < f.y { f.y - y } else { (y - (f.y + f.altura)).max(0.0) };
        let f = pag.fatias.iter().min_by(|a, b| fora(a).total_cmp(&fora(b)).then(dist(a).total_cmp(&dist(b))))?;
        let x = x - f.x;
        let lay = &self.layouts[f.bloco];
        let (mut li, mut topo) = (f.de, f.y);
        for k in f.de..f.ate {
            li = k;
            if y < topo + lay.linhas[k].altura {
                break;
            }
            topo += lay.linhas[k].altura;
        }
        Some(self.pos_na_linha(f.bloco, li, x, y - topo))
    }

    /// O título para onde leva a linha do sumário em (`x`, `y`) da página, se o ponto cai numa.
    pub fn alvo_do_sumario(&self, pagina: usize, y: f32) -> Option<u64> {
        let y = y - self.doc.pagina.margem_sup;
        let f = self.paginas.get(pagina)?.fatias.iter().find(|f| (f.y..f.y + f.altura).contains(&y))?;
        let mut topo = f.y;
        for linha in &self.layouts[f.bloco].linhas[f.de..f.ate] {
            if y < topo + linha.altura {
                return linha.alvo;
            }
            topo += linha.altura;
        }
        None
    }

    /// O ponto da linha `li` do bloco `i` mais perto de `x` (e de `y_na_linha`, para células de várias linhas).
    pub(crate) fn pos_na_linha(&self, i: usize, li: usize, x: f32, y_na_linha: f32) -> Pos {
        let b = &self.doc.blocos[i];
        let linha = &self.layouts[i].linhas[li];
        match &b.conteudo {
            c if eh_atomico(c) => Pos { bloco: b.id, celula: None, desl: 0 },
            Conteudo::Tabela { .. } => {
                let col = linha.celulas.iter().rposition(|c| c.x0 - 5.0 <= x).unwrap_or(0);
                let col = b.mesclagens.iter()
                    .find(|m| m.contem(li, col))
                    .map_or(col, |m| m.coluna);
                let celula = Some((li as u32, col as u32));
                let todos = self.pedacos_do_alvo(i, li, celula);
                let grupo = linha_da_celula(&todos, y_na_linha);
                Pos { bloco: b.id, celula, desl: desl_no_x(&grupo, x, 0) }
            }
            _ => Pos { bloco: b.id, celula: None, desl: desl_no_x(&self.pedacos_do_alvo(i, li, None), x, linha.de) },
        }
    }

    /// `x` do cursor relativo à área de texto (o que ↑ e ↓ procuram).
    pub(crate) fn x_do_cursor(&self, p: &Pos) -> f32 {
        let Some(i) = self.idx(p.bloco) else { return 0.0 };
        if eh_atomico(&self.doc.blocos[i].conteudo) {
            return 0.0;
        }
        self.ponto_na_linha(i, self.linha_do_pos(i, p), p.celula, p.desl).0
    }

    /// O ponto uma linha acima (`-1`) ou abaixo (`1`), na mesma posição horizontal `x`.
    pub(crate) fn pos_vertical(&self, foco: &Pos, sentido: i32, x: f32) -> Pos {
        let Some(i) = self.idx(foco.bloco) else { return *foco };
        let n = self.layouts[i].linhas.len();
        let li = self.linha_do_pos(i, foco);
        let alvo = li as i64 + i64::from(sentido);
        if (0..n as i64).contains(&alvo) && !eh_atomico(&self.doc.blocos[i].conteudo) {
            return self.pos_na_linha(i, alvo as usize, x, f32::MAX);
        }
        let vizinho = i as i64 + i64::from(sentido);
        match usize::try_from(vizinho).ok().filter(|v| *v < self.doc.blocos.len()) {
            Some(v) => {
                let linhas = self.layouts[v].linhas.len();
                self.pos_na_linha(v, if sentido < 0 { linhas - 1 } else { 0 }, x, if sentido < 0 { f32::MAX } else { 0.0 })
            }
            None => *foco,
        }
    }

    /// O começo (`false`) ou o fim (`true`) da linha visual em que o ponto está.
    pub(crate) fn extremo_da_linha(&self, p: &Pos, fim: bool) -> Pos {
        let Some(i) = self.idx(p.bloco) else { return *p };
        let c = &self.doc.blocos[i].conteudo;
        if eh_atomico(c) {
            return *p;
        }
        let total = tamanho_alvo(c, p.celula);
        if p.celula.is_some() {
            return Pos { desl: if fim { total } else { 0 }, ..*p };
        }
        let li = self.linha_do_pos(i, p);
        let linhas = &self.layouts[i].linhas;
        let desl = if !fim {
            linhas[li].de
        } else if li + 1 < linhas.len() {
            linhas[li].pedacos.last().map_or(linhas[li].de, |u| u.de + u.caracteres())
        } else {
            total
        };
        Pos { desl, ..*p }
    }

    /// O cursor, a seleção (só nas páginas `de..=ate`, as que estão à vista) e o formato no cursor.
    pub fn visao(&self, de: usize, ate: usize) -> Visao {
        Visao { cursor: self.geometria(&self.cursor.foco), retangulos: self.retangulos_da_selecao(de, ate), achados: self.retangulos_dos_achados(de, ate), formato: self.formato_do_cursor() }
    }

    /// Os retângulos da seleção nas páginas `de..=ate`.
    pub fn retangulos_da_selecao(&self, de: usize, ate: usize) -> Vec<Ret> {
        let (ini, fim) = self.selecao_ordenada();
        let (Some(i0), Some(i1)) = (self.idx(ini.bloco), self.idx(fim.bloco)) else { return Vec::new() };
        let mut saida = Vec::new();
        if ini == fim {
            return saida;
        }
        for i in i0..=i1 {
            let faixa = (if i == i0 { Some(ini) } else { None }, if i == i1 { Some(fim) } else { None });
            self.retangulos_do_bloco(i, faixa, i < i1, de..=ate, &mut saida);
        }
        saida
    }

    pub(crate) fn retangulos_do_bloco(&self, i: usize, faixa: (Option<Pos>, Option<Pos>), continua: bool, paginas: std::ops::RangeInclusive<usize>, saida: &mut Vec<Ret>) {
        let cfg = self.doc.pagina;
        let c = &self.doc.blocos[i].conteudo;
        let inteiro = eh_atomico(c) || matches!(c, Conteudo::Tabela { .. }) && faixa.0.map(|p| p.celula) != faixa.1.map(|p| p.celula);
        let total = tamanho_alvo(c, faixa.0.and_then(|p| p.celula));
        let (a, b) = (faixa.0.map_or(0, |p| p.desl), faixa.1.map_or(total, |p| p.desl));
        for (pagina, f) in self.fatias_do_bloco(i) {
            if !paginas.contains(&pagina) {
                continue;
            }
            let mut topo = cfg.margem_sup + f.y;
            for li in f.de..f.ate {
                let linha = &self.layouts[i].linhas[li];
                let celula = faixa.0.and_then(|p| p.celula);
                let sel = Selecionado { a, b, total, continua, celula };
                // Seleção dentro de uma célula: só a linha da tabela dela (cada `Linha` é uma linha da tabela).
                let r = if inteiro { Some((0.0, self.doc.colunas().1)) }
                    else if celula.is_some_and(|(l, _)| l as usize != li) { None }
                    else { self.faixa_da_linha(i, li, &sel) };
                if let Some((x0, x1)) = r {
                    saida.push(Ret { pagina, x: cfg.margem_esq + f.x + x0, y: topo, w: (x1 - x0).max(2.0), h: linha.altura });
                }
                topo += linha.altura;
            }
        }
    }

    /// O trecho horizontal da linha `li` coberto pela seleção `[a, b)`; `None` se a linha não é tocada.
    fn faixa_da_linha(&self, i: usize, li: usize, sel: &Selecionado) -> Option<(f32, f32)> {
        let Selecionado { a, b, total, continua, celula } = *sel;
        let linha: &Linha = &self.layouts[i].linhas[li];
        let (ls, le) = if celula.is_some() { (0, total) } else { (linha.de, self.layouts[i].linhas.get(li + 1).map_or(total, |n| n.de)) };
        let ultima = celula.is_some() || li + 1 == self.layouts[i].linhas.len();
        let de = a.max(ls);
        let ate = b.min(le);
        let vazia = ls == le;
        if de > ate || (de == ate && !(vazia && continua) && !(ultima && continua && de == total && ls < le)) {
            return None;
        }
        let padrao = (linha.x0, linha.base, linha.tam);
        let pedacos = self.pedacos_do_alvo(i, li, celula);
        let x0 = x_na_linha(&pedacos, de, padrao).0;
        let mut x1 = x_na_linha(&pedacos, ate, padrao).0;
        if continua && ultima && ate == total {
            x1 += largura(" ", 0, linha.tam.max(8.0));
        }
        Some((x0, x1))
    }

    /// A seleção em ordem do documento: `(começo, fim)`.
    pub(crate) fn selecao_ordenada(&self) -> (Pos, Pos) {
        let (a, f) = (self.cursor.ancora, self.cursor.foco);
        if self.chave(&a) <= self.chave(&f) {
            (a, f)
        } else {
            (f, a)
        }
    }

    /// Para comparar posições na ordem do documento.
    pub(crate) fn chave(&self, p: &Pos) -> (usize, (u32, u32), usize) {
        (self.idx(p.bloco).unwrap_or(0), p.celula.unwrap_or((0, 0)), p.desl)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Relato do usuário: um clique duplo numa célula pintava a mesma coluna de todas as linhas.
    #[test]
    fn selecao_numa_celula_pinta_so_a_celula_alinhada_a_direita() {
        let mut s = Sessao::com_documento(crate::modelos::criar("escolar").unwrap());
        let i = s.doc.blocos.iter().position(|b| matches!(b.conteudo, Conteudo::Tabela { .. })).unwrap();
        let (pagina, f) = s.fatias_do_bloco(i)[0];
        let (de, y) = (f.de, f.y);
        let cfg = s.doc.pagina;
        let p = s.layouts[i].linhas[de].pedacos.iter().find(|p| p.celula == Some(0)).unwrap().clone();
        assert!(p.x > 50.0, "o modelo alinha a coluna 0 à direita");
        s.clicar(pagina, cfg.margem_esq + p.x + 3.0, cfg.margem_sup + y + p.y - 3.0, false, 2);
        let rets = s.retangulos_da_selecao(0, s.paginas.len());
        assert_eq!(rets.len(), 1, "{rets:?}");
        let w = crate::medida::largura(&p.texto, p.estilo, p.tamanho);
        assert!((rets[0].x - (cfg.margem_esq + p.x)).abs() < 0.01 && (rets[0].w - w).abs() < 0.01, "{rets:?}");
        let g = s.geometria(&s.cursor.foco).unwrap();
        assert!((g.x - (cfg.margem_esq + p.x + w)).abs() < 0.01, "o cursor no fim do texto desenhado");
    }
}
