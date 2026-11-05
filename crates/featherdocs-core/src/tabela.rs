//! A aba contextual Tabela: inserir e excluir linha e coluna, excluir a tabela, ligar a
//! linha de cabeçalho. Cada operação é um passo de desfazer.

use crate::cursor::{Cursor, Pos};
use crate::modelo::{Bloco, Conteudo, MesclagemTabela};
use crate::sessao::{Mudanca, Sessao, Splice};

/// As células de um pedaço de planilha copiado (FeatherSheet, Excel, LibreOffice: colunas
/// separadas por tabulação, uma linha por linha), se o texto é um.
/// Pede o mesmo número de colunas (2 a 30) em todas as linhas: texto sem tabulação nunca é grade.
/// Fora de uma tabela, [`Sessao::colar_grade`] pede ainda duas linhas ou mais, para uma linha com
/// uma tabulação solta ("Nome:\tAna") continuar texto.
// shortcut: a célula do Excel com quebra de linha (entre aspas) não é reconhecida e cola como texto; tratar se alguém reclamar.
pub(crate) fn grade_colada(texto: &str) -> Option<Vec<Vec<String>>> {
    let texto = texto.strip_suffix('\n').unwrap_or(texto);
    let grade: Vec<Vec<String>> = texto.split('\n').map(|l| l.split('\t').map(|c| c.trim().to_string()).collect()).collect();
    let n = grade.first()?.len();
    (grade.len() <= 200 && (2..=30).contains(&n) && grade.iter().all(|l| l.len() == n)).then_some(grade)
}

impl Sessao {
    /// Cola a grade de [`grade_colada`] (um Ctrl+Z). Na tabela, preenche a partir da célula do
    /// cursor, com linhas novas embaixo se faltar; fora dela, vira uma tabela nova com cabeçalho.
    /// O que não é grade de verdade para o lugar (uma linha fora de tabela, a tabela mesclada)
    /// cola como texto.
    pub(crate) fn colar_grade(&mut self, grade: Vec<Vec<String>>, original: &str) -> Mudanca {
        let celula = |v: &str| if v.is_empty() { Vec::new() } else { vec![crate::modelo::Trecho::simples(v)] };
        if let Some((i, l, k)) = self.celula_do_cursor() {
            if !self.doc.blocos[i].mesclagens.is_empty() || self.doc.blocos[i].tabela.tem_mescla_vertical() {
                return self.inserir_texto(original);
            }
            return self.mudar_tabela(i, (l, k), |linhas, _, props| {
                let colunas = linhas.iter().map(Vec::len).max().unwrap_or(1);
                for (dl, valores) in grade.iter().enumerate() {
                    let r = l + dl;
                    if r >= linhas.len() {
                        linhas.push(vec![Vec::new(); colunas]);
                        props.inserir_linha(r);
                    }
                    for (dk, v) in valores.iter().enumerate() {
                        if let Some(c) = linhas[r].get_mut(k + dk) {
                            *c = celula(v);
                        }
                    }
                }
            });
        }
        let cru = self.idx(self.cursor.foco.bloco).is_some_and(|i| crate::alvo::eh_texto_cru(&self.doc.blocos[i].conteudo));
        if grade.len() < 2 || cru {
            return self.inserir_texto(original);
        }
        let linhas = grade.iter().map(|l| l.iter().map(|v| celula(v)).collect()).collect();
        self.inserir_conteudo(Conteudo::Tabela { cabecalho: true, linhas })
    }

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

    /// Aplica o estilo pronto `ESTILOS_DE_TABELA[n]` à tabela do cursor, num Ctrl+Z: fios, fundo do
    /// cabeçalho e das linhas listradas, e o texto do cabeçalho em negrito (branco no fundo escuro).
    /// As cores ficam nas células (não num estilo do Word), então a tela, o .fdoz e o .docx mostram igual.
    // shortcut: linha nova inserida depois não herda a listra; reaplicar o estilo corrige.
    pub fn tabela_estilo_aplicar(&mut self, n: usize) -> Mudanca {
        use crate::modelo::{BordasDeTabela, FioDeBorda, NEGRITO};
        let (Some((i, _, _)), Some(e)) = (self.celula_do_cursor(), ESTILOS_DE_TABELA.get(n)) else { return self.sem_mudanca() };
        let mut b = self.doc.blocos[i].clone();
        let Conteudo::Tabela { cabecalho, linhas } = &mut b.conteudo else { return self.sem_mudanca() };
        let fio = |cor: u32| Some(FioDeBorda { estilo: "single".into(), espessura: 4, cor });
        let nada = || Some(FioDeBorda { estilo: "nil".into(), espessura: 0, cor: 0 });
        let (fora_h, fora_v, dentro_h, dentro_v) = match e.fios {
            Fios::Grade(c) => (fio(c), fio(c), fio(c), fio(c)),
            Fios::Horizontais(c) => (fio(c), nada(), fio(c), nada()),
            Fios::Contorno(c) => (fio(c), fio(c), nada(), nada()),
            Fios::Nenhum => (nada(), nada(), nada(), nada()),
        };
        b.tabela.bordas = BordasDeTabela { cima: fora_h.clone(), baixo: fora_h, esquerda: fora_v.clone(), direita: fora_v, dentro_h, dentro_v };
        *cabecalho = e.cabeca != 0 || e.negrito_no_cabecalho;
        let com_cabeca = *cabecalho;
        for (l, linha) in linhas.iter_mut().enumerate() {
            let cabeca = l == 0 && com_cabeca;
            for (k, celula) in linha.iter_mut().enumerate() {
                let p = b.tabela.celula_mut(l, k);
                p.bordas = BordasDeTabela::default();
                p.fundo = if cabeca { e.cabeca } else if (l + usize::from(!com_cabeca)) % 2 == 0 { e.listra } else { 0 };
                if l == 0 {
                    for t in celula.iter_mut() {
                        if cabeca {
                            t.estilo |= NEGRITO;
                        }
                        // O branco só fica sobre fundo escuro; trocando de estilo, o texto volta à cor automática.
                        if e.texto_claro && cabeca {
                            t.fmt.cor = BRANCO;
                        } else if t.fmt.cor == BRANCO {
                            t.fmt.cor = 0;
                        }
                    }
                }
            }
        }
        let cursor = self.cursor;
        self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }

    /// Volta a tabela do cursor à grade simples, sem fundos nem fios próprios das células (a
    /// tabela colada de outro programa com formatação escondida).
    pub fn tabela_formatacao_limpar(&mut self) -> Mudanca {
        let Some((i, _, _)) = self.celula_do_cursor() else { return self.sem_mudanca() };
        self.mudar_props(i, |p| {
            p.bordas = crate::modelo::BordasDeTabela::default();
            p.celulas.iter_mut().for_each(|c| (c.fundo, c.bordas) = (0, Default::default()));
        })
    }

    /// A tabela do cursor tem linha de cabeçalho?
    pub fn tabela_tem_cabecalho(&self) -> bool {
        self.celula_do_cursor().is_some_and(|(i, _, _)| matches!(self.doc.blocos[i].conteudo, Conteudo::Tabela { cabecalho: true, .. }))
    }
}

const BRANCO: u32 = 0xffff_ffff;

/// Os fios de um estilo pronto de tabela (a cor em RGBA; 0 = automática).
#[derive(Clone, Copy, Debug)]
pub enum Fios {
    Grade(u32),
    /// Só as linhas horizontais (em cima, embaixo e entre as linhas).
    Horizontais(u32),
    Contorno(u32),
    Nenhum,
}

/// Um estilo pronto da galeria da aba Tabela.
#[derive(Clone, Copy, Debug)]
pub struct EstiloTabela {
    pub nome: &'static str,
    /// Fundo da linha de cabeçalho (0 = sem fundo).
    pub cabeca: u32,
    /// Texto branco no cabeçalho (fundo escuro).
    pub texto_claro: bool,
    /// Fundo das linhas alternadas (0 = sem listras).
    pub listra: u32,
    pub fios: Fios,
    /// Cabeçalho em negrito mesmo sem fundo.
    pub negrito_no_cabecalho: bool,
}

const fn estilo(nome: &'static str, cabeca: u32, texto_claro: bool, listra: u32, fios: Fios, negrito_no_cabecalho: bool) -> EstiloTabela {
    EstiloTabela { nome, cabeca, texto_claro, listra, fios, negrito_no_cabecalho }
}

pub const ESTILOS_DE_TABELA: [EstiloTabela; 8] = [
    estilo("Grade simples", 0, false, 0, Fios::Grade(0), false),
    estilo("Cabeçalho azul", 0x2c75_abff, true, 0, Fios::Grade(0xbfd4_e6ff), false),
    estilo("Listrada azul", 0x2c75_abff, true, 0xe8f1_fbff, Fios::Horizontais(0xbfd4_e6ff), false),
    estilo("Listrada verde", 0x2d9b_71ff, true, 0xe6f4_eaff, Fios::Horizontais(0xb7df_c6ff), false),
    estilo("Escura", 0x2632_38ff, true, 0xf1f3_f4ff, Fios::Nenhum, false),
    estilo("Minimalista", 0, false, 0, Fios::Horizontais(0x9e9e_9eff), true),
    estilo("Laranja", 0xe473_3eff, true, 0xfdee_e6ff, Fios::Contorno(0xe473_3eff), false),
    estilo("Sem bordas", 0, false, 0, Fios::Nenhum, true),
];

#[cfg(test)]
mod testes {
    #[test]
    fn imagem_na_celula_e_desenhada_e_a_linha_cresce() {
        use crate::layout::Deco;
        use crate::modelo::{Documento, Marca, Trecho};
        let imagem = Trecho::objeto(Marca::Imagem { origem: "data:image/png;base64,AA==".into(), largura: 60.0, altura: 50.0 });
        let mut d = Documento::vazio();
        d.blocos.clear();
        d.acrescentar(Conteudo::Tabela { cabecalho: false, linhas: vec![vec![vec![imagem, Trecho::simples("\u{2028}legenda")], vec![Trecho::simples("texto")]]] });
        let s = Sessao::com_documento(d);
        let linha = &s.layouts[0].linhas[0];
        assert!(linha.deco.iter().any(|x| matches!(x, Deco::Imagem { h, .. } if *h == 50.0)), "a imagem sai no quadro");
        assert!(linha.altura > 60.0, "a linha cresce até a imagem e a legenda: {}", linha.altura);
    }

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
    fn estilo_pronto_pinta_cabecalho_e_listras_num_ctrl_z() {
        let mut s = com_tabela();
        s.tabela_linha(true);
        s.tabela_linha(true);
        let antes = s.documento().clone();
        s.tabela_estilo_aplicar(2);
        let b = &s.documento().blocos[0];
        let Conteudo::Tabela { cabecalho, linhas } = &b.conteudo else { panic!() };
        assert!(*cabecalho);
        let fundo = |l| b.tabela.celula(l, 0).map_or(0, |c| c.fundo);
        assert_eq!((fundo(0), fundo(1), fundo(2), fundo(3)), (0x2c75_abff, 0, 0xe8f1_fbff, 0));
        assert!(linhas[0][0].iter().all(|t| t.fmt.cor == BRANCO && t.estilo & crate::modelo::NEGRITO != 0));
        assert_eq!(b.tabela.bordas.dentro_v.as_ref().map(|f| f.estilo.as_str()), Some("nil"));
        // Trocar para um estilo claro devolve a cor automática ao texto do cabeçalho.
        s.tabela_estilo_aplicar(5);
        let Conteudo::Tabela { linhas, .. } = &s.documento().blocos[0].conteudo else { panic!() };
        assert!(linhas[0][0].iter().all(|t| t.fmt.cor == 0));
        s.tabela_formatacao_limpar();
        assert!(s.documento().blocos[0].tabela.celulas.iter().all(|c| c.fundo == 0));
        s.desfazer();
        s.desfazer();
        s.desfazer();
        assert_eq!(s.documento(), &antes);
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
