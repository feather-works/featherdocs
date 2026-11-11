//! Mudar blocos de lugar: Alt+Shift+↑/↓ sobe ou desce os parágrafos da seleção, o painel
//! Estrutura leva um título com tudo o que está abaixo dele, e Ordenar põe os parágrafos
//! selecionados em ordem alfabética. Cada movimento é um Ctrl+Z.

use crate::cursor::{Cursor, Pos};
use crate::layout::texto_do_titulo;
use crate::modelo::Conteudo;
use crate::sessao::{Mudanca, Sessao, Splice};

/// A chave de ordenar um texto: sem maiúsculas nem acentos, e os números com zeros à esquerda,
/// para "item 2" vir antes de "item 10".
fn chave_de_ordem(texto: &str) -> String {
    let dobrado = crate::bibliografia::dobrar(&texto.trim().to_lowercase());
    let mut saida = String::with_capacity(dobrado.len());
    let mut numero = String::new();
    for c in dobrado.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            numero.push(c);
            continue;
        }
        if !numero.is_empty() {
            saida.push_str(&format!("{:0>20}", std::mem::take(&mut numero)));
        }
        if c != '\0' {
            saida.push(c);
        }
    }
    saida
}

impl Sessao {
    /// Sobe (`para_cima`) ou desce um parágrafo os blocos que a seleção toca.
    pub fn mover_paragrafos(&mut self, para_cima: bool) -> Mudanca {
        let (ini, fim) = self.selecao_ordenada();
        let (Some(i0), Some(i1)) = (self.idx(ini.bloco), self.idx(fim.bloco)) else { return self.sem_mudanca() };
        // A seleção que termina no começo do parágrafo seguinte (Shift+↓ numa linha) não o leva junto.
        let i1 = if fim.desl == 0 && fim.celula.is_none() && i1 > i0 { i1 - 1 } else { i1.max(i0) };
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

    /// Ordena de A a Z (ou de Z a A) os parágrafos e itens de lista que a seleção toca, sem
    /// diferenciar maiúsculas nem acentos; os vazios vão para o fim. Um Ctrl+Z.
    pub fn ordenar_paragrafos(&mut self, decrescente: bool) -> Result<Mudanca, String> {
        let (ini, fim) = self.selecao_ordenada();
        let (Some(i0), Some(i1)) = (self.idx(ini.bloco), self.idx(fim.bloco)) else { return Ok(self.sem_mudanca()) };
        let i1 = if fim.desl == 0 && fim.celula.is_none() && i1 > i0 { i1 - 1 } else { i1.max(i0) };
        if i1 == i0 {
            return Err("Selecione dois ou mais parágrafos para ordenar.".into());
        }
        // A sugestão só registra o que muda dentro do bloco: uma troca de ordem entraria sem marca.
        if self.rastreando() {
            return Err("Com o modo Sugerir ligado a nova ordem não ficaria marcada: desligue para ordenar.".into());
        }
        let blocos = &self.doc.blocos[i0..=i1];
        if !blocos.iter().all(|b| matches!(b.conteudo, Conteudo::Paragrafo { .. } | Conteudo::Item { .. })) {
            return Err("Só parágrafos e itens de lista se ordenam: tire da seleção títulos, tabelas e imagens.".into());
        }
        let niveis = |b: &crate::modelo::Bloco| if let Conteudo::Item { nivel, .. } = b.conteudo { nivel } else { 0 };
        if blocos.iter().any(|b| niveis(b) != niveis(&blocos[0])) {
            return Err("A lista tem subitens: ordene cada nível separado, selecionando só os itens de um nível.".into());
        }
        // A chave sem os objetos presos ao texto e sem o que foi sugerido apagar.
        let chave = |b: &crate::modelo::Bloco| {
            let texto: String = b.conteudo.textos()[0].iter()
                .filter(|t| t.marca.is_none() && !t.rev.as_ref().is_some_and(|r| r.tipo == crate::modelo::TipoDeRevisao::Remocao))
                .map(|t| t.texto.as_str()).collect();
            chave_de_ordem(&texto)
        };
        let mut com_chave: Vec<(String, crate::modelo::Bloco)> = blocos.iter().map(|b| (chave(b), b.clone())).collect();
        com_chave.sort_by(|(a, _), (b, _)| {
            let ordem = if decrescente { b.cmp(a) } else { a.cmp(b) };
            a.is_empty().cmp(&b.is_empty()).then(ordem)
        });
        let mut dentro: Vec<crate::modelo::Bloco> = com_chave.into_iter().map(|(_, b)| b).collect();
        if dentro.iter().map(|b| b.id).eq(blocos.iter().map(|b| b.id)) {
            return Ok(self.sem_mudanca());
        }
        // "Recomeçar em 1" e o número inicial são da posição (o começo da lista), não do item.
        let (reiniciar, inicio) = (blocos[0].atrib.reiniciar, blocos[0].atrib.mais.inicio);
        for b in &mut dentro {
            (b.atrib.reiniciar, b.atrib.mais.inicio) = (false, None);
        }
        (dentro[0].atrib.reiniciar, dentro[0].atrib.mais.inicio) = (reiniciar, inicio);
        // Os mesmos parágrafos continuam selecionados, agora em ordem.
        let ultimo = &dentro[dentro.len() - 1];
        let ancora = Pos { bloco: dentro[0].id, celula: None, desl: 0 };
        let foco = Pos { bloco: ultimo.id, celula: None, desl: crate::alvo::tamanho_alvo(&ultimo.conteudo, None) };
        let cursor = Cursor { ancora, foco, ..Cursor::em(foco) };
        Ok(self.transacao(Splice { pos: i0, quantos: i1 + 1 - i0, dentro }, cursor, None))
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
    fn ordenar_de_a_a_z_sem_acento_com_numeros_e_desfazer() {
        let mut s = doc(&[(0, "banana"), (0, ""), (0, "Água"), (0, "item 10"), (0, "item 2")]);
        s.selecionar_tudo();
        s.ordenar_paragrafos(false).unwrap();
        assert_eq!(ordem(&s), [3, 1, 5, 4, 2], "Água, banana, item 2, item 10 e o vazio no fim");
        s.ordenar_paragrafos(true).unwrap();
        assert_eq!(ordem(&s), [4, 5, 1, 3, 2], "de Z a A, o vazio continua no fim");
        s.desfazer();
        assert_eq!(ordem(&s), [3, 1, 5, 4, 2]);
        s.ir_para_bloco(1);
        assert!(s.ordenar_paragrafos(false).is_err(), "um parágrafo só");
        let mut t = doc(&[(1, "Título"), (0, "b"), (0, "a")]);
        t.selecionar_tudo();
        assert!(t.ordenar_paragrafos(false).is_err(), "título na seleção");
        // A lista que recomeça em 1: a marca fica no primeiro item, seja ele qual for.
        let mut u = doc(&[(0, "b"), (0, "a")]);
        let mut d = u.documento().clone();
        d.blocos[0].atrib.reiniciar = true;
        u = Sessao::com_documento(d);
        u.selecionar_tudo();
        u.ordenar_paragrafos(false).unwrap();
        assert_eq!(ordem(&u), [2, 1]);
        assert!(u.documento().blocos[0].atrib.reiniciar && !u.documento().blocos[1].atrib.reiniciar);
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
    fn selecao_ate_o_comeco_do_seguinte_nao_leva_o_seguinte() {
        let mut s = doc(&[(0, "a"), (0, "b"), (0, "c")]);
        s.tecla("ArrowDown", false, true, false);
        assert_eq!((s.cursor().foco.bloco, s.cursor().foco.desl), (2, 0), "a seleção vai até o começo do b");
        s.mover_paragrafos(false);
        assert_eq!(ordem(&s), [2, 1, 3], "só o primeiro desce");
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
