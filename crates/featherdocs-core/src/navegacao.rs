//! Mover o cursor e a seleção: setas, palavras, linhas, páginas, clique, duplo e
//! triplo clique. Nada aqui muda o documento.

use crate::alvo::{caracteres_alvo, dimensoes, eh_atomico, primeiro_alvo, tamanho_alvo, ultimo_alvo};
use crate::cursor::{Cursor, Movimento, Pos};
use crate::modelo::Conteudo;
use crate::sessao::Sessao;
use crate::texto::{palavra_anterior, palavra_em, proxima_palavra};

impl Sessao {
    /// O primeiro ponto do bloco `i`: o começo do texto, a primeira célula, ou o próprio bloco.
    pub(crate) fn inicio_do_bloco(&self, i: usize) -> Pos {
        let b = &self.doc.blocos[i];
        Pos { bloco: b.id, celula: primeiro_alvo(&b.conteudo).flatten(), desl: 0 }
    }

    /// O último ponto do bloco `i`.
    pub(crate) fn fim_do_bloco(&self, i: usize) -> Pos {
        let b = &self.doc.blocos[i];
        let celula = ultimo_alvo(&b.conteudo).flatten();
        Pos { bloco: b.id, celula, desl: tamanho_alvo(&b.conteudo, celula) }
    }

    /// Um caractere à frente, passando para a célula ou o bloco seguinte no fim.
    pub(crate) fn pos_seguinte(&self, p: &Pos) -> Pos {
        let Some(i) = self.idx(p.bloco) else { return *p };
        let c = &self.doc.blocos[i].conteudo;
        if !eh_atomico(c) && p.desl < tamanho_alvo(c, p.celula) {
            return Pos { desl: p.desl + 1, ..*p };
        }
        if let (Some((l, k)), Some((linhas, colunas))) = (p.celula, dimensoes(c)) {
            let (l, k) = (l as usize, k as usize);
            let proxima_coluna = self.doc.blocos[i].mesclagens.iter()
                .find(|m| m.linha == l && m.coluna == k)
                .map_or(k.saturating_add(1), |m| k.saturating_add(m.colunas));
            let proxima = if proxima_coluna < colunas { Some((l, proxima_coluna)) } else if l + 1 < linhas { Some((l + 1, 0)) } else { None };
            if let Some((l, k)) = proxima {
                return Pos { celula: Some((l as u32, k as u32)), desl: 0, ..*p };
            }
        }
        if i + 1 < self.doc.blocos.len() {
            self.inicio_do_bloco(i + 1)
        } else {
            self.fim_do_bloco(i)
        }
    }

    /// Um caractere atrás, passando para a célula ou o bloco anterior no começo.
    pub(crate) fn pos_anterior(&self, p: &Pos) -> Pos {
        let Some(i) = self.idx(p.bloco) else { return *p };
        let c = &self.doc.blocos[i].conteudo;
        if !eh_atomico(c) && p.desl > 0 {
            return Pos { desl: p.desl - 1, ..*p };
        }
        if let (Some((l, k)), Some((_, colunas))) = (p.celula, dimensoes(c)) {
            let anterior = if k > 0 { Some((l, k - 1)) } else if l > 0 { Some((l - 1, colunas as u32 - 1)) } else { None };
            // A célula coberta por uma mesclagem é a do começo dela (como em `pos_seguinte`).
            let anterior = anterior.map(|(l, k)| {
                let m = self.doc.blocos[i].mesclagens.iter().find(|m| m.linha == l as usize && (m.coluna..m.coluna + m.colunas).contains(&(k as usize)));
                (l, m.map_or(k, |m| m.coluna as u32))
            });
            if let Some(cel) = anterior {
                return Pos { celula: Some(cel), desl: tamanho_alvo(c, Some(cel)), ..*p };
            }
        }
        if i > 0 {
            self.fim_do_bloco(i - 1)
        } else {
            self.inicio_do_bloco(0)
        }
    }

    /// Põe o foco em `foco`: com `estender` a seleção cresce, senão o cursor vira um ponto só.
    pub(crate) fn definir_foco(&mut self, foco: Pos, estender: bool, x_desejado: Option<f32>) {
        let mut foco = foco;
        if let Some((linha, coluna)) = foco.celula {
            if let Some(i) = self.idx(foco.bloco) {
                if let Some(m) = self.doc.blocos[i].mesclagens.iter()
                    .find(|m| m.contem(linha as usize, coluna as usize))
                {
                    foco.celula = Some((linha, m.coluna as u32));
                    if coluna as usize != m.coluna {
                        foco.desl = 0;
                    }
                }
            }
        }
        if estender {
            let a = self.cursor.ancora;
            // Dentro de uma célula a seleção não passa para outra célula (a tabela inteira entra só quando sai dela).
            if a.celula.is_some() && foco.bloco == a.bloco && foco.celula != a.celula {
                let total = self.idx(a.bloco).map_or(0, |i| tamanho_alvo(&self.doc.blocos[i].conteudo, a.celula));
                let depois = self.chave(&foco) > self.chave(&a);
                foco = Pos { celula: a.celula, desl: if depois { total } else { 0 }, ..a };
            }
        }
        self.cursor = Cursor { ancora: if estender { self.cursor.ancora } else { foco }, foco, x_desejado, pendente: None };
        self.digitando = None;
    }

    /// Anda com o cursor. `estender` (Shift) cresce a seleção.
    pub fn mover(&mut self, m: Movimento, estender: bool) {
        let foco = self.cursor.foco;
        if !estender && !self.cursor.vazio() && matches!(m, Movimento::Esquerda | Movimento::Direita) {
            let (a, b) = self.selecao_ordenada();
            self.definir_foco(if m == Movimento::Esquerda { a } else { b }, false, None);
            return;
        }
        let (destino, x) = match m {
            Movimento::Esquerda => (self.pos_anterior(&foco), None),
            Movimento::Direita => (self.pos_seguinte(&foco), None),
            Movimento::PalavraEsquerda => (self.pos_palavra(&foco, false), None),
            Movimento::PalavraDireita => (self.pos_palavra(&foco, true), None),
            Movimento::Cima | Movimento::Baixo => {
                let x = self.cursor.x_desejado.unwrap_or_else(|| self.x_do_cursor(&foco));
                (self.pos_vertical(&foco, if m == Movimento::Cima { -1 } else { 1 }, x), Some(x))
            }
            Movimento::InicioDaLinha => (self.extremo_da_linha(&foco, false), None),
            Movimento::FimDaLinha => (self.extremo_da_linha(&foco, true), None),
            Movimento::InicioDoDocumento => (self.inicio_do_bloco(0), None),
            Movimento::FimDoDocumento => (self.fim_do_bloco(self.doc.blocos.len() - 1), None),
            Movimento::PaginaAcima | Movimento::PaginaAbaixo => (self.pos_de_outra_pagina(&foco, m == Movimento::PaginaAbaixo), self.cursor.x_desejado),
        };
        self.definir_foco(destino, estender, x);
    }

    fn pos_palavra(&self, p: &Pos, para_frente: bool) -> Pos {
        let Some(i) = self.idx(p.bloco) else { return *p };
        let c = &self.doc.blocos[i].conteudo;
        let total = tamanho_alvo(c, p.celula);
        if eh_atomico(c) || (para_frente && p.desl >= total) {
            return if para_frente { self.pos_seguinte(p) } else { self.pos_anterior(p) };
        }
        if !para_frente && p.desl == 0 {
            return self.pos_anterior(p);
        }
        let texto = caracteres_alvo(c, p.celula);
        let desl = if para_frente { proxima_palavra(&texto, p.desl) } else { palavra_anterior(&texto, p.desl) };
        Pos { desl, ..*p }
    }

    /// PgUp/PgDn: o ponto do mesmo lugar, uma página adiante.
    fn pos_de_outra_pagina(&self, p: &Pos, abaixo: bool) -> Pos {
        let Some(g) = self.geometria(p) else { return *p };
        let alvo = if abaixo { (g.pagina + 1).min(self.paginas.len() - 1) } else { g.pagina.saturating_sub(1) };
        self.pos_na_pagina(alvo, g.x, g.y + g.h / 2.0).unwrap_or(*p)
    }

    /// Clique: `contagem` 1 põe o cursor, 2 seleciona a palavra, 3 o parágrafo; `estender` (Shift ou arrasto) cresce a seleção.
    pub fn clicar(&mut self, pagina: usize, x: f32, y: f32, estender: bool, contagem: u8) {
        let Some(p) = self.pos_na_pagina(pagina, x, y) else { return };
        let Some(i) = self.idx(p.bloco) else { return };
        let c = &self.doc.blocos[i].conteudo;
        if contagem < 2 || eh_atomico(c) || matches!(c, Conteudo::Tabela { .. }) && p.celula.is_none() {
            self.definir_foco(p, estender, None);
            return;
        }
        let (a, b) = if contagem == 2 { palavra_em(&caracteres_alvo(c, p.celula), p.desl) } else { (0, tamanho_alvo(c, p.celula)) };
        self.definir_foco(Pos { desl: a, ..p }, false, None);
        self.cursor.foco = Pos { desl: b, ..p };
    }

    /// O botão direito: dentro da seleção, ela fica (o menu age sobre ela); fora, o cursor vai para o ponto, como no Word.
    pub fn clicar_direito(&mut self, pagina: usize, x: f32, y: f32) {
        let Some(p) = self.pos_na_pagina(pagina, x, y) else { return };
        if !self.cursor.vazio() {
            let (ini, fim) = self.selecao_ordenada();
            let k = self.chave(&p);
            if self.chave(&ini) <= k && k <= self.chave(&fim) {
                return;
            }
        }
        self.definir_foco(p, false, None);
    }

    /// Põe o cursor no começo do bloco `id` (o painel de estrutura, o sumário).
    pub fn ir_para_bloco(&mut self, id: u64) {
        if let Some(i) = self.idx(id) {
            let p = self.inicio_do_bloco(i);
            self.definir_foco(p, false, None);
        }
    }

    /// Ctrl+A.
    pub fn selecionar_tudo(&mut self) {
        let (a, f) = (self.inicio_do_bloco(0), self.fim_do_bloco(self.doc.blocos.len() - 1));
        self.cursor = Cursor { ancora: a, foco: f, x_desejado: None, pendente: None };
        self.digitando = None;
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::Documento;

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    #[test]
    fn direita_e_esquerda_passam_de_um_paragrafo_para_o_outro() {
        let mut s = doc(&["ab", "cd"]);
        for _ in 0..3 {
            s.mover(Movimento::Direita, false);
        }
        let f = s.cursor.foco;
        assert_eq!((s.idx(f.bloco), f.desl), (Some(1), 0));
        s.mover(Movimento::Esquerda, false);
        assert_eq!((s.idx(s.cursor.foco.bloco), s.cursor.foco.desl), (Some(0), 2));
    }

    #[test]
    fn shift_estende_e_seta_sem_shift_colapsa_na_ponta() {
        let mut s = doc(&["abcdef"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, true);
        s.mover(Movimento::Direita, true);
        assert_eq!((s.cursor.ancora.desl, s.cursor.foco.desl), (1, 3));
        s.mover(Movimento::Esquerda, false);
        assert!(s.cursor.vazio());
        assert_eq!(s.cursor.foco.desl, 1);
    }

    #[test]
    fn home_end_e_documento() {
        let mut s = doc(&["um dois", "tres"]);
        s.mover(Movimento::FimDoDocumento, false);
        assert_eq!((s.idx(s.cursor.foco.bloco), s.cursor.foco.desl), (Some(1), 4));
        s.mover(Movimento::InicioDaLinha, false);
        assert_eq!(s.cursor.foco.desl, 0);
        s.mover(Movimento::InicioDoDocumento, false);
        s.mover(Movimento::FimDaLinha, false);
        assert_eq!(s.cursor.foco.desl, 7);
    }

    #[test]
    fn cima_e_baixo_lembram_a_coluna() {
        let mut s = doc(&["abcdefghij", "ab", "abcdefghij"]);
        for _ in 0..8 {
            s.mover(Movimento::Direita, false);
        }
        s.mover(Movimento::Baixo, false);
        assert_eq!(s.cursor.foco.desl, 2, "o parágrafo curto limita");
        s.mover(Movimento::Baixo, false);
        assert_eq!(s.cursor.foco.desl, 8, "volta à coluna de origem");
        s.mover(Movimento::Cima, false);
        s.mover(Movimento::Cima, false);
        assert_eq!((s.idx(s.cursor.foco.bloco), s.cursor.foco.desl), (Some(0), 8));
    }

    #[test]
    fn palavras_e_duplo_clique() {
        let mut s = doc(&["um dois tres"]);
        s.mover(Movimento::PalavraDireita, false);
        assert_eq!(s.cursor.foco.desl, 3);
        s.mover(Movimento::PalavraDireita, false);
        assert_eq!(s.cursor.foco.desl, 8);
        s.mover(Movimento::PalavraEsquerda, false);
        assert_eq!(s.cursor.foco.desl, 3);
        // clique duplo no meio de "dois": x ≈ 3 espaços + 2 letras
        let g = s.geometria(&Pos { desl: 5, ..s.cursor.foco }).unwrap();
        s.clicar(g.pagina, g.x, g.y + 5.0, false, 2);
        assert_eq!((s.cursor.ancora.desl, s.cursor.foco.desl), (3, 7));
        s.clicar(g.pagina, g.x, g.y + 5.0, false, 3);
        assert_eq!((s.cursor.ancora.desl, s.cursor.foco.desl), (0, 12));
    }

    #[test]
    fn botao_direito_guarda_a_selecao_so_quando_cai_dentro_dela() {
        let mut s = doc(&["um dois tres"]);
        let g = |s: &Sessao, desl| s.geometria(&Pos { desl, ..s.cursor.foco }).unwrap();
        let (g2, g8) = (g(&s, 2), g(&s, 8));
        s.clicar(g2.pagina, g2.x + 0.01, g2.y + 5.0, false, 1);
        s.clicar(g8.pagina, g8.x + 0.01, g8.y + 5.0, true, 1);
        let g5 = g(&s, 5);
        s.clicar_direito(g5.pagina, g5.x + 0.01, g5.y + 5.0);
        assert_eq!((s.cursor.ancora.desl, s.cursor.foco.desl), (2, 8), "dentro: a seleção fica");
        let g10 = g(&s, 10);
        s.clicar_direito(g10.pagina, g10.x + 0.01, g10.y + 5.0);
        assert!(s.cursor.vazio());
        assert_eq!(s.cursor.foco.desl, 10, "fora: o cursor vai para o clique");
    }

    #[test]
    fn clique_poe_o_cursor_entre_as_letras_onde_o_motor_desenhou() {
        let mut s = doc(&["abcdef"]);
        for desl in 0..=6 {
            let g = s.geometria(&Pos { desl, ..s.cursor.foco }).unwrap();
            s.clicar(g.pagina, g.x + 0.01, g.y + g.h / 2.0, false, 1);
            assert_eq!(s.cursor.foco.desl, desl, "clicando exatamente onde o cursor {desl} é desenhado");
        }
    }
}
