//! A aba contextual Tabela: inserir e excluir linha e coluna, excluir a tabela, ligar a
//! linha de cabeçalho. Cada operação é um passo de desfazer.

use crate::cursor::{Cursor, Pos};
use crate::modelo::{Bloco, Conteudo, MesclagemTabela};
use crate::sessao::{Mudanca, Sessao, Splice};

impl Sessao {
    /// A tabela e a célula do cursor: (índice do bloco, linha, coluna).
    fn celula_do_cursor(&self) -> Option<(usize, usize, usize)> {
        let f = self.cursor.foco;
        let (l, k) = f.celula?;
        let i = self.idx(f.bloco)?;
        matches!(self.doc.blocos[i].conteudo, Conteudo::Tabela { .. }).then_some((i, l as usize, k as usize))
    }

    /// Troca a tabela do bloco `i` pela versão mudada por `f` e põe o cursor na célula `(l, k)`.
    fn mudar_tabela(&mut self, i: usize, cel: (usize, usize), f: impl FnOnce(&mut Vec<Vec<crate::modelo::Celula>>, &mut bool, &mut crate::modelo::PropsTabela)) -> Mudanca {
        let mut b = self.doc.blocos[i].clone();
        let Conteudo::Tabela { linhas, cabecalho } = &mut b.conteudo else { return self.sem_mudanca() };
        f(linhas, cabecalho, &mut b.tabela);
        let l = cel.0.min(linhas.len().saturating_sub(1));
        let k = cel.1.min(linhas.get(l).map_or(1, Vec::len).saturating_sub(1));
        let cursor = Cursor::em(Pos { bloco: b.id, celula: Some((l as u32, k as u32)), desl: 0 });
        self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }

    /// Substitui o conteúdo tabular já validado por um adaptador externo,
    /// preservando identidade e atributos do bloco (incluindo bordas,
    /// alinhamento e legendas). Uma única transação garante Ctrl+Z/Ctrl+Y
    /// e recalcula a paginação, inclusive tabelas entre várias páginas.
    ///
    /// O motor do Docs permanece dono da edição rica, FDOZ e composição.
    pub fn tabela_conteudo_substituir(
        &mut self,
        id: u64,
        conteudo: Conteudo,
        celula: (usize, usize),
    ) -> Result<Mudanca, String> {
        self.tabela_conteudo_substituir_com(id, conteudo, celula, |_| {})
    }

    /// Como `tabela_conteudo_substituir`, deslocando antes as propriedades da tabela (larguras,
    /// fundos, alturas) para a forma nova: quem inseriu ou excluiu uma linha ou coluna diz como.
    pub fn tabela_conteudo_substituir_com(
        &mut self,
        id: u64,
        conteudo: Conteudo,
        celula: (usize, usize),
        mudar_props: impl FnOnce(&mut crate::modelo::PropsTabela),
    ) -> Result<Mudanca, String> {
        let Conteudo::Tabela { linhas, .. } = &conteudo else {
            return Err("O conteúdo fornecido não é uma tabela.".into());
        };
        let colunas = linhas.first().map_or(0, Vec::len);
        if !(1..=200).contains(&linhas.len())
            || !(1..=30).contains(&colunas)
            || linhas.iter().any(|linha| linha.len() != colunas)
        {
            return Err("A tabela deve ser retangular (1–200 linhas e 1–30 colunas).".into());
        }
        let i = self.idx(id).ok_or("O bloco não existe mais.")?;
        let original = &self.doc.blocos[i];
        if !matches!(original.conteudo, Conteudo::Tabela { .. }) {
            return Err("O bloco não é uma tabela.".into());
        }
        if !original.mesclagens.is_empty() {
            let Conteudo::Tabela { linhas: antigas, .. } = &original.conteudo else {
                unreachable!();
            };
            let old_width = antigas.first().map_or(0, Vec::len);
            if antigas.len() != linhas.len() || old_width != colunas {
                return Err("Desmescle as células antes de alterar linhas ou colunas.".into());
            }
            for mescla in &original.mesclagens {
                let fim = mescla.coluna.checked_add(mescla.colunas)
                    .ok_or("A mesclagem tem índices inválidos.")?;
                if mescla.linha >= linhas.len() || fim > colunas {
                    return Err("A mesclagem tem índices fora da tabela.".into());
                }
                for coluna in mescla.coluna + 1..fim {
                    if !linhas[mescla.linha][coluna].is_empty() {
                        return Err("Não se pode editar uma célula encoberta pela mesclagem.".into());
                    }
                }
            }
        }
        if original.conteudo == conteudo {
            return Ok(self.sem_mudanca());
        }
        let altura = linhas.len();
        let mut bloco = original.clone();
        bloco.conteudo = conteudo;
        mudar_props(&mut bloco.tabela);
        bloco.tabela.ajustar(altura, colunas);
        let alvo = Cursor::em(Pos {
            bloco: id,
            celula: Some((
                celula.0.min(altura - 1) as u32,
                celula.1.min(colunas - 1) as u32,
            )),
            desl: 0,
        });
        Ok(self.transacao(
            Splice { pos: i, quantos: 1, dentro: vec![bloco] },
            alvo,
            None,
        ))
    }

    /// Aplica ou remove mesclagens horizontais como uma transação reversível.
    /// Não apaga nem combina trechos: as células encobertas devem estar vazias.
    pub fn tabela_mesclagens_definir(
        &mut self,
        id: u64,
        mesclagens: Vec<MesclagemTabela>,
        celula: (usize, usize),
    ) -> Result<Mudanca, String> {
        let indice = self.idx(id).ok_or("A tabela não existe.")?;
        let original = &self.doc.blocos[indice];
        let Conteudo::Tabela { linhas, .. } = &original.conteudo else {
            return Err("O bloco não é uma tabela.".into());
        };
        let colunas = linhas.first().map_or(0, Vec::len);
        if colunas == 0 || linhas.is_empty()
            || linhas.iter().any(|linha| linha.len() != colunas) {
            return Err("A tabela não é retangular.".into());
        }
        let mut ocupadas = std::collections::HashSet::new();
        for m in &mesclagens {
            if m.colunas < 2 || m.linha >= linhas.len()
                || m.coluna.checked_add(m.colunas).map_or(true, |fim| fim > colunas) {
                return Err("A região de mesclagem está fora da tabela.".into());
            }
            for c in m.coluna..m.coluna + m.colunas {
                if !ocupadas.insert((m.linha, c)) {
                    return Err("Mesclagens sobrepostas não são permitidas.".into());
                }
                if c != m.coluna && !linhas[m.linha][c].is_empty() {
                    return Err("Não é possível mesclar: outra célula contém texto ou objetos.".into());
                }
            }
        }
        if original.mesclagens == mesclagens {
            return Ok(self.sem_mudanca());
        }
        let mut novo = original.clone();
        novo.mesclagens = mesclagens;
        let cursor = Cursor::em(Pos {
            bloco: id,
            celula: Some((
                celula.0.min(linhas.len() - 1) as u32,
                celula.1.min(colunas - 1) as u32,
            )),
            desl: 0,
        });
        Ok(self.transacao(Splice { pos: indice, quantos: 1, dentro: vec![novo] }, cursor, None))
    }

    /// Uma linha vazia acima (`abaixo = false`) ou abaixo da linha do cursor.
    pub fn tabela_linha(&mut self, abaixo: bool) -> Mudanca {
        let Some((i, l, k)) = self.celula_do_cursor() else { return self.sem_mudanca() };
        if !self.doc.blocos[i].mesclagens.is_empty() || self.doc.blocos[i].tabela.tem_mescla_vertical() {
            return self.sem_mudanca(); // exigir desmesclar antes de alterar estrutura
        }
        let nova = if abaixo { l + 1 } else { l };
        self.mudar_tabela(i, (nova, k), |linhas, _, props| {
            let colunas = linhas.iter().map(Vec::len).max().unwrap_or(1);
            linhas.insert(nova, vec![Vec::new(); colunas]);
            props.inserir_linha(nova);
        })
    }

    /// Uma coluna vazia à esquerda (`direita = false`) ou à direita da coluna do cursor.
    pub fn tabela_coluna(&mut self, direita: bool) -> Mudanca {
        let Some((i, l, k)) = self.celula_do_cursor() else { return self.sem_mudanca() };
        if !self.doc.blocos[i].mesclagens.is_empty() || self.doc.blocos[i].tabela.tem_mescla_vertical() {
            return self.sem_mudanca(); // não deslocar mesclagem sem atualizar a região
        }
        let nova = if direita { k + 1 } else { k };
        self.mudar_tabela(i, (l, nova), |linhas, _, props| {
            for r in linhas.iter_mut() {
                let at = nova.min(r.len());
                r.insert(at, Vec::new());
            }
            props.inserir_coluna(nova);
        })
    }

    /// Exclui a `linha`, a `coluna` ou a `tabela` do cursor. A última linha ou coluna leva a tabela junto.
    pub fn tabela_excluir(&mut self, o_que: &str) -> Result<Mudanca, String> {
        let Some((i, l, k)) = self.celula_do_cursor() else { return Ok(self.sem_mudanca()) };
        let Conteudo::Tabela { linhas, .. } = &self.doc.blocos[i].conteudo else { return Ok(self.sem_mudanca()) };
        let (n_linhas, n_colunas) = (linhas.len(), linhas.iter().map(Vec::len).max().unwrap_or(1));
        if o_que != "tabela" && (!self.doc.blocos[i].mesclagens.is_empty() || self.doc.blocos[i].tabela.tem_mescla_vertical()) {
            return Err("Desmescle as células antes de excluir linhas ou colunas.".into());
        }
        Ok(match o_que {
            "linha" if n_linhas > 1 => self.mudar_tabela(i, (l, k), |linhas, _, props| {
                linhas.remove(l);
                props.remover_linha(l);
            }),
            "coluna" if n_colunas > 1 => self.mudar_tabela(i, (l, k), |linhas, _, props| {
                for r in linhas.iter_mut().filter(|r| k < r.len()) {
                    r.remove(k);
                }
                props.remover_coluna(k);
            }),
            "linha" | "coluna" | "tabela" => self.excluir_tabela(i),
            _ => return Err(format!("Não sei excluir \"{o_que}\" da tabela.")),
        })
    }

    fn excluir_tabela(&mut self, i: usize) -> Mudanca {
        let p = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
        let cursor = Cursor::em(Pos { bloco: p.id, celula: None, desl: 0 });
        let dentro: Vec<Bloco> = vec![p];
        self.transacao(Splice { pos: i, quantos: 1, dentro }, cursor, None)
    }

    /// Liga ou desliga a linha de cabeçalho (a primeira linha em negrito, com fundo).
    pub fn tabela_cabecalho(&mut self) -> Mudanca {
        let Some((i, l, k)) = self.celula_do_cursor() else { return self.sem_mudanca() };
        self.mudar_tabela(i, (l, k), |_, cabecalho, _| *cabecalho = !*cabecalho)
    }

    /// As células escolhidas: o retângulo entre a âncora e o foco, se os dois estão na mesma
    /// tabela; senão só a do cursor. (índice do bloco, células como (linha, coluna)).
    fn celulas_escolhidas(&self) -> Option<(usize, Vec<(usize, usize)>)> {
        let (i, l, k) = self.celula_do_cursor()?;
        let a = self.cursor.ancora;
        let (l0, k0) = match a.celula {
            Some((al, ak)) if a.bloco == self.cursor.foco.bloco => (al as usize, ak as usize),
            _ => (l, k),
        };
        let cel = (l.min(l0)..=l.max(l0)).flat_map(|x| (k.min(k0)..=k.max(k0)).map(move |y| (x, y))).collect();
        Some((i, cel))
    }

    /// Troca as propriedades da tabela do bloco `i` sem mexer no texto nem no cursor (um passo de desfazer).
    fn mudar_props(&mut self, i: usize, f: impl FnOnce(&mut crate::modelo::PropsTabela)) -> Mudanca {
        let mut b = self.doc.blocos[i].clone();
        f(&mut b.tabela);
        if b.tabela == self.doc.blocos[i].tabela {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }

    /// Muda as propriedades (fundo, alinhamento…) das células escolhidas.
    pub fn tabela_celulas_definir(&mut self, f: impl Fn(&mut crate::modelo::PropsCelula)) -> Mudanca {
        let Some((i, cel)) = self.celulas_escolhidas() else { return self.sem_mudanca() };
        self.mudar_props(i, |p| cel.iter().for_each(|&(l, k)| f(p.celula_mut(l, k))))
    }

    /// As propriedades da célula do cursor, se ela tem alguma.
    pub fn tabela_celula_do_cursor(&self) -> Option<&crate::modelo::PropsCelula> {
        let (i, l, k) = self.celula_do_cursor()?;
        self.doc.blocos[i].tabela.celula(l, k)
    }

    /// As bordas da tabela inteira: `todas`, `nenhuma` ou `externas` (o contorno, sem as de dentro).
    /// As bordas próprias das células saem, para a escolha valer na tabela toda.
    pub fn tabela_bordas_definir(&mut self, qual: &str) -> Result<Mudanca, String> {
        use crate::modelo::{BordasDeTabela, FioDeBorda};
        let Some((i, _, _)) = self.celula_do_cursor() else { return Ok(self.sem_mudanca()) };
        let fio = |cheio: bool| Some(FioDeBorda { estilo: if cheio { "single" } else { "nil" }.into(), espessura: if cheio { 4 } else { 0 }, cor: 0 });
        let (fora, dentro) = match qual {
            "todas" => (true, true),
            "nenhuma" => (false, false),
            "externas" => (true, false),
            _ => return Err(format!("Não conheço as bordas \"{qual}\".")),
        };
        Ok(self.mudar_props(i, |p| {
            p.bordas = BordasDeTabela { cima: fio(fora), esquerda: fio(fora), baixo: fio(fora), direita: fio(fora), dentro_h: fio(dentro), dentro_v: fio(dentro) };
            p.celulas.iter_mut().for_each(|c| c.bordas = BordasDeTabela::default());
        }))
    }

    /// A tabela do cursor tem linha de cabeçalho?
    pub fn tabela_tem_cabecalho(&self) -> bool {
        self.celula_do_cursor().is_some_and(|(i, _, _)| matches!(self.doc.blocos[i].conteudo, Conteudo::Tabela { cabecalho: true, .. }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::Documento;

    fn com_tabela() -> Sessao {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: Vec::new() });
        let mut s = Sessao::com_documento(d);
        s.inserir_tabela(2, 2).unwrap();
        s
    }

    fn medidas(s: &Sessao) -> (usize, usize) {
        let Conteudo::Tabela { linhas, .. } = &s.documento().blocos[0].conteudo else { panic!("não é tabela") };
        (linhas.len(), linhas.iter().map(Vec::len).max().unwrap_or(0))
    }

    #[test]
    fn inserir_e_excluir_linhas_e_colunas_com_desfazer() {
        let mut s = com_tabela();
        s.tabela_linha(true);
        s.tabela_coluna(false);
        assert_eq!(medidas(&s), (3, 3));
        s.tabela_excluir("coluna").unwrap();
        assert_eq!(medidas(&s), (3, 2));
        s.tabela_excluir("linha").unwrap();
        assert_eq!(medidas(&s), (2, 2));
        s.desfazer();
        assert_eq!(medidas(&s), (3, 2));
        assert!(s.tabela_excluir("célula").is_err());
    }

    #[test]
    fn propriedades_andam_com_a_linha_e_a_coluna() {
        use crate::modelo::{AlturaDeLinha, PropsTabela};
        let mut p = PropsTabela { colunas: vec![50.0, 150.0], ..PropsTabela::default() };
        p.celula_mut(1, 1).fundo = 0xFF0000FF;
        p.alturas.push(AlturaDeLinha { linha: 1, altura: 20.0, exata: false });
        p.inserir_linha(0);
        p.inserir_coluna(0);
        assert_eq!(p.celula(2, 2).map(|c| c.fundo), Some(0xFF0000FF));
        assert_eq!(p.altura(2).map(|a| a.altura), Some(20.0));
        assert_eq!(p.colunas, vec![50.0, 50.0, 150.0]);
        p.remover_coluna(2);
        assert!(p.celulas.is_empty(), "a coluna excluída leva as propriedades das células dela");
        p.remover_linha(0);
        assert_eq!(p.altura(1).map(|a| a.altura), Some(20.0));
        p.ajustar(1, 2);
        assert!(p.alturas.is_empty());
    }

    #[test]
    fn fundo_alinhamento_e_bordas_com_desfazer() {
        use crate::modelo::Alinhamento;
        let mut s = com_tabela();
        s.tabela_celulas_definir(|c| { c.fundo = 0x00FF00FF; c.alinhamento = Alinhamento::Centro; c.vertical = 1 });
        let c = s.tabela_celula_do_cursor().unwrap();
        assert_eq!((c.fundo, c.alinhamento, c.vertical), (0x00FF00FF, Alinhamento::Centro, 1));
        s.tabela_bordas_definir("externas").unwrap();
        let b = &s.documento().blocos[0].tabela.bordas;
        assert_eq!((b.cima.as_ref().unwrap().estilo.as_str(), b.dentro_h.as_ref().unwrap().estilo.as_str()), ("single", "nil"));
        s.desfazer();
        assert!(s.documento().blocos[0].tabela.bordas.eh_padrao());
        s.desfazer();
        assert!(s.tabela_celula_do_cursor().is_none());
        assert!(s.tabela_bordas_definir("rosa").is_err());
    }

    #[test]
    fn cabecalho_alterna_e_excluir_tabela_deixa_paragrafo() {
        let mut s = com_tabela();
        assert!(s.tabela_tem_cabecalho());
        s.tabela_cabecalho();
        assert!(!s.tabela_tem_cabecalho());
        s.tabela_excluir("tabela").unwrap();
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Paragrafo { .. }));
    }
}
