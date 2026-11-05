//! Mudar blocos de lugar: Alt+Shift+↑/↓ sobe ou desce os parágrafos da seleção, e o painel
//! Estrutura leva um título com tudo o que está abaixo dele. Cada movimento é um Ctrl+Z.

use crate::layout::texto_do_titulo;
use crate::sessao::{Mudanca, Sessao, Splice};

impl Sessao {
    /// Sobe (`para_cima`) ou desce um parágrafo os blocos que a seleção toca.
    pub fn mover_paragrafos(&mut self, para_cima: bool) -> Mudanca {
        let (ini, fim) = self.selecao_ordenada();
        let (Some(i0), Some(i1)) = (self.idx(ini.bloco), self.idx(fim.bloco)) else { return self.sem_mudanca() };
        let i1 = i1.max(i0);
        let destino = if para_cima { i0.checked_sub(1) } else { (i1 + 2 <= self.doc.blocos.len()).then_some(i1 + 2) };
        match destino {
            Some(d) => self.mover_faixa(i0, i1, d),
            None => self.sem_mudanca(),
        }
    }

    /// Leva o título `id` e a seção dele (até o próximo título do mesmo nível ou acima) para antes
    /// do bloco `antes_de` (`None` = para o fim do documento).
    pub fn mover_secao(&mut self, id: u64, antes_de: Option<u64>) -> Mudanca {
        let Some((i0, i1)) = self.idx(id).and_then(|i| Some((i, self.fim_da_secao(i)?))) else { return self.sem_mudanca() };
        let destino = match antes_de {
            Some(a) => match self.idx(a) {
                Some(d) => d,
                None => return self.sem_mudanca(),
            },
            None => self.doc.blocos.len(),
        };
        self.mover_faixa(i0, i1, destino)
    }

    /// O último bloco da seção que o título `i` abre (até o próximo título do mesmo nível ou acima).
    pub(crate) fn fim_da_secao(&self, i: usize) -> Option<usize> {
        let (nivel, _) = texto_do_titulo(&self.doc.blocos[i])?;
        let fim = self.doc.blocos[i + 1..].iter().position(|b| texto_do_titulo(b).is_some_and(|(n, _)| n <= nivel));
        Some(fim.map_or(self.doc.blocos.len(), |k| i + 1 + k) - 1)
    }

    /// Os blocos `[i0, i1]` passam a ficar antes do bloco `destino` (contado antes de mover).
    fn mover_faixa(&mut self, i0: usize, i1: usize, destino: usize) -> Mudanca {
        // Para dentro de si mesma (ou para onde já está) não muda nada.
        if (i0..=i1 + 1).contains(&destino) {
            return self.sem_mudanca();
        }
        let (lo, hi) = (i0.min(destino), (i1 + 1).max(destino));
        let mut dentro = self.doc.blocos[lo..hi].to_vec();
        if destino < i0 {
            dentro[..].rotate_right(i1 + 1 - i0);
        } else {
            dentro[..].rotate_left(i1 + 1 - i0);
        }
        let cursor = self.cursor;
        self.transacao(Splice { pos: lo, quantos: hi - lo, dentro }, cursor, None)
    }
}

#[cfg(test)]
mod testes {
    use crate::modelo::{Bloco, Conteudo, Documento, Trecho};
    use crate::sessao::Sessao;

    fn doc(partes: &[(u8, &str)]) -> Sessao {
        let blocos = partes
            .iter()
            .enumerate()
            .map(|(i, &(nivel, t))| {
                let texto = vec![Trecho::simples(t)];
                Bloco::novo(i as u64 + 1, if nivel > 0 { Conteudo::Titulo { nivel, texto } } else { Conteudo::Paragrafo { texto } })
            })
            .collect();
        Sessao::com_documento(Documento { blocos, ..Documento::default() })
    }

    fn ordem(s: &Sessao) -> Vec<u64> {
        s.documento().blocos.iter().map(|b| b.id).collect()
    }

    #[test]
    fn alt_shift_seta_sobe_e_desce_e_desfaz() {
        let mut s = doc(&[(0, "a"), (0, "b"), (0, "c")]);
        s.ir_para_bloco(2);
        s.mover_paragrafos(true);
        assert_eq!(ordem(&s), [2, 1, 3]);
        s.mover_paragrafos(true);
        assert_eq!(ordem(&s), [2, 1, 3], "no topo não sobe mais");
        s.mover_paragrafos(false);
        s.mover_paragrafos(false);
        assert_eq!(ordem(&s), [1, 3, 2]);
        assert_eq!(s.cursor().foco.bloco, 2, "o cursor vai junto");
        s.desfazer();
        assert_eq!(ordem(&s), [1, 2, 3]);
    }

    #[test]
    fn secao_vai_com_os_subtitulos_e_o_texto() {
        // 1 Intro, 2 texto, 3 Métodos, 4 Sub, 5 texto, 6 Fim
        let mut s = doc(&[(1, "Intro"), (0, "t"), (1, "Métodos"), (2, "Sub"), (0, "t"), (1, "Fim")]);
        s.mover_secao(3, Some(1));
        assert_eq!(ordem(&s), [3, 4, 5, 1, 2, 6]);
        s.mover_secao(1, None);
        assert_eq!(ordem(&s), [3, 4, 5, 6, 1, 2]);
        s.mover_secao(3, Some(4));
        assert_eq!(ordem(&s), [3, 4, 5, 6, 1, 2], "para dentro de si mesma não muda");
    }

    #[test]
    fn estrutura_conta_as_palavras_de_cada_secao() {
        let s = doc(&[(1, "Intro"), (0, "um dois"), (2, "Sub"), (0, "três"), (1, "Fim"), (0, "quatro cinco seis")]);
        let p: Vec<usize> = s.estrutura().iter().map(|t| t.palavras).collect();
        assert_eq!(p, [4, 1, 3], "a seção conta o subtítulo e o texto dele");
    }
}
