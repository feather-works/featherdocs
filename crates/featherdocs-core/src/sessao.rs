//! A sessão: o documento aberto, o cursor, a composição guardada de cada bloco, as
//! páginas e o desfazer. Toda edição é um *splice* (troca de um trecho da lista de
//! blocos), então desfazer custa o tamanho do que mudou, não do documento.
//!
//! Custo de uma tecla: recompor **só o bloco mexido** (o resto vem do `cache`) e
//! rodar a paginação, que é uma conta por linha sem medir texto nenhum.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

use crate::alvo::{eh_atomico, primeiro_alvo};
use crate::cursor::{Cursor, Pos};
use crate::edicao::{tabela_de_texto, tabela_em_texto, trechos_de};
use crate::inline::texto_puro;
use crate::layout::{lista_gerada, sumario, texto_do_titulo, EntradaSumario, Entorno, LayoutBloco};
use crate::modelo::{Atrib, Bloco, ConfigPagina, Conteudo, Documento, Estilo, Estilos};
use crate::paginacao::{alinhar_na_vertical, marcadores, numeros, paginar, Pagina};

const MAXIMO_DESFAZER: usize = 2000;

/// Troca `quantos` blocos a partir de `pos` por `dentro`.
pub(crate) struct Splice {
    pub pos: usize,
    pub quantos: usize,
    pub dentro: Vec<Bloco>,
}

/// Um passo de desfazer: o splice que desfaz, o cursor para onde voltar e, se a edição mexeu
/// nos estilos com nome ou na numeração, como eles estavam.
struct Passo {
    s: Splice,
    cursor: Cursor,
    estilos: Option<Estilos>,
}

/// O que, além do próprio bloco, decide a composição dele: guardado junto, para saber quando refazer.
type Assinatura = (Option<String>, Option<Estilo>);

/// O que uma edição mudou: a versão nova e onde.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Mudanca {
    pub versao: u64,
    pub paginas: usize,
    /// Os blocos tocados (os `id`s, para a interface saber o que redesenhar).
    pub afetados: Vec<u64>,
    /// A primeira página que pode ter mudado (0, 1…): as de antes dela seguem como estavam.
    pub primeira_pagina: usize,
}

/// Um título do documento para o painel de estrutura.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Titulo {
    pub id: u64,
    pub nivel: u8,
    pub texto: String,
    pub pagina: usize,
}

/// O resumo que a barra de status e os menus precisam.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Estado {
    pub versao: u64,
    pub paginas: usize,
    pub blocos: usize,
    pub palavras: usize,
    pub caracteres: usize,
    pub pode_desfazer: bool,
    pub pode_refazer: bool,
    pub sujo: bool,
    pub titulo: String,
    pub largura: f32,
    pub altura: f32,
    pub margem_esq: f32,
    pub margem_dir: f32,
    pub margem_sup: f32,
    pub margem_inf: f32,
    /// `a4`, `carta`, `oficio`, `a3`, `a5` ou `personalizado`.
    pub papel: String,
    pub paisagem: bool,
    /// Marcas de formatação (¶ ·) à vista.
    pub marcas: bool,
    /// Linhas de grade à vista.
    pub grade: bool,
    /// Cabeçalho, rodapé e plano de fundo (para as caixas da faixa virem preenchidas).
    pub extras: crate::modelo::Extras,
    /// Os estilos com nome (para a galeria) e a numeração dos títulos.
    pub estilos: Estilos,
    /// O estilo das citações (para o menu marcar o atual).
    pub estilo_citacao: crate::modelo::EstiloDeCitacao,
    /// Revisão → Modo: `editando`, `sugerindo` ou `leitura`.
    pub modo_edicao: String,
    /// Há sugestões à espera (para Aceitar e Rejeitar).
    pub tem_revisoes: bool,
}

/// A cópia guardada dentro do aplicativo, com a formatação (o texto puro vai para a área de transferência do sistema).
pub(crate) struct AreaInterna {
    pub plano: String,
    pub blocos: Vec<Conteudo>,
}

/// O documento aberto.
pub struct Sessao {
    pub(crate) doc: Documento,
    pub(crate) cache: HashMap<u64, (Assinatura, Arc<LayoutBloco>)>,
    /// Blocos que ninguém editou mas foram recompostos (o número do título ou o estilo mudou): também se repintam.
    pub(crate) recompostos: Vec<u64>,
    /// O número de cada bloco (só os títulos numerados têm).
    pub(crate) numeros: Vec<Option<String>>,
    rotulo_capitulo: String,
    /// As palavras das legendas (figura, tabela, equação) e os títulos das listas (de figuras, de tabelas).
    rotulos_legenda: [String; 3],
    rotulos_lista: [String; 2],
    pub(crate) layouts: Vec<Arc<LayoutBloco>>,
    pub(crate) paginas: Vec<Pagina>,
    pub(crate) indices: HashMap<u64, usize>,
    pub(crate) primeira_pagina_do_bloco: Vec<usize>,
    pub(crate) versao: u64,
    versao_gravada: u64,
    desfazer: Vec<Passo>,
    refazer: Vec<Passo>,
    titulo_na_pagina: HashMap<u64, usize>,
    rotulo_sumario: String,
    pub(crate) cursor: Cursor,
    /// O bloco e o tipo (digitar, apagar para trás, apagar para frente) da última edição contínua: a próxima, igual, vira o mesmo passo de desfazer.
    pub(crate) digitando: Option<(u64, u8)>,
    pub(crate) area: Option<AreaInterna>,
    pub(crate) pincel: Option<crate::fonte::Pincel>,
    pub(crate) busca: Option<crate::busca::Busca>,
    /// Marcas de formatação (¶ e ·) à vista.
    pub(crate) marcas: bool,
    /// Linhas de grade sobre a área do texto.
    pub(crate) grade: bool,
    /// A data de hoje como a interface escreve (o campo `data`); o motor não lê relógio.
    pub(crate) hoje: String,
    /// O nome do arquivo aberto (o campo `arquivo`).
    pub(crate) arquivo: String,
    /// O título das notas de fim.
    pub(crate) rotulo_notas: String,
    /// Os títulos da lista de referências e do índice remissivo.
    pub(crate) rotulo_referencias: String,
    pub(crate) rotulo_indice: String,
    /// As entradas do índice remissivo com as páginas da última paginação.
    pub(crate) indice: Vec<crate::indice::EntradaDeIndice>,
    /// Revisão → Modo: somente leitura (não se grava no arquivo).
    pub(crate) somente_leitura: bool,
    /// Quem está editando (o autor das sugestões e dos comentários).
    pub(crate) autor: String,
    /// O controle de alterações está suspenso (aceitar, rejeitar, comentar).
    pub(crate) rastreio_pausado: bool,
    /// Comentando: a edição passa mesmo com o documento travado.
    pub(crate) comentando: bool,
}

impl Default for Sessao {
    fn default() -> Self {
        Self::com_documento(Documento::default())
    }
}

impl Sessao {
    /// Abre um documento já montado (lido de arquivo ou novo), com o cursor no começo.
    pub fn com_documento(mut doc: Documento) -> Self {
        doc.acertar_ids();
        if doc.blocos.is_empty() {
            doc.acrescentar(Conteudo::Paragrafo { texto: Vec::new() });
        }
        let inicio = Pos { bloco: doc.blocos[0].id, celula: primeiro_alvo(&doc.blocos[0].conteudo).flatten(), desl: 0 };
        let mut s = Self {
            doc,
            cache: HashMap::new(),
            recompostos: Vec::new(),
            numeros: Vec::new(),
            rotulo_capitulo: "Capítulo".to_string(),
            rotulos_legenda: ["Figura".to_string(), "Tabela".to_string(), "Equação".to_string()],
            rotulos_lista: ["Lista de figuras".to_string(), "Lista de tabelas".to_string()],
            layouts: Vec::new(),
            paginas: Vec::new(),
            indices: HashMap::new(),
            primeira_pagina_do_bloco: Vec::new(),
            versao: 1,
            versao_gravada: 1,
            desfazer: Vec::new(),
            refazer: Vec::new(),
            titulo_na_pagina: HashMap::new(),
            rotulo_sumario: "Sumário".to_string(),
            cursor: Cursor::em(inicio),
            digitando: None,
            area: None,
            pincel: None,
            busca: None,
            marcas: false,
            grade: false,
            hoje: String::new(),
            arquivo: String::new(),
            rotulo_notas: "Notas".to_string(),
            rotulo_referencias: "Referências".to_string(),
            rotulo_indice: "Índice".to_string(),
            indice: Vec::new(),
            somente_leitura: false,
            autor: String::new(),
            rastreio_pausado: false,
            comentando: false,
        };
        s.repaginar();
        s
    }

    /// O cursor e a seleção.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// O documento (para gravar em arquivo).
    pub fn documento(&self) -> &Documento {
        &self.doc
    }

    /// O texto do título do sumário.
    pub fn rotulo_do_sumario(&self) -> String {
        self.rotulo_sumario.clone()
    }

    /// Troca o título do sumário (vem da língua da interface).
    pub fn rotulo_do_sumario_definir(&mut self, rotulo: &str) {
        if self.rotulo_sumario != rotulo {
            self.rotulo_sumario = rotulo.to_string();
            self.repaginar();
        }
    }

    /// A palavra antes do número do capítulo (vem da língua da interface).
    pub fn rotulo_do_capitulo_definir(&mut self, rotulo: &str) {
        if self.rotulo_capitulo != rotulo {
            self.rotulo_capitulo = rotulo.to_string();
            self.repaginar();
            self.recompostos.clear();
        }
    }

    /// As palavras das legendas e os títulos das listas (vêm da língua da interface).
    pub fn rotulos_de_legenda_definir(&mut self, legendas: [String; 3], listas: [String; 2]) {
        if self.rotulos_legenda != legendas || self.rotulos_lista != listas {
            self.rotulos_legenda = legendas;
            self.rotulos_lista = listas;
            self.repaginar();
            self.recompostos.clear();
        }
    }

    /// Compõe o que falta e reparte em páginas. Um bloco cujo número ou estilo mudou é recomposto
    /// (e entra em `recompostos`, para a tela repintar a página dele).
    pub(crate) fn repaginar(&mut self) {
        self.rotular();
        self.compor_e_paginar();
        // O campo de página e as páginas do índice só se sabem depois de paginar; se mudaram, compõe
        // de novo (até três voltas: o índice novo pode empurrar páginas).
        for _ in 0..3 {
            let paginas = self.rotular_paginas();
            let indice = self.atualizar_indice();
            if !paginas && !indice {
                break;
            }
            self.compor_e_paginar();
        }
    }

    fn compor_e_paginar(&mut self) {
        let limite = self.doc.pagina.largura_util();
        crate::layout::composicao_tex(self.doc.extras.composicao_tex);
        self.numeros = numeros(&self.doc.blocos, self.doc.estilos.numeracao, &self.rotulo_capitulo, &self.rotulos_legenda);
        let entradas = self.entradas_do_sumario();
        // Documento do LaTeX: as listas geradas levam o título como um `\section*` e o texto na família do corpo.
        let secao = self.doc.blocos.iter().find_map(|b| match b.conteudo {
            Conteudo::Titulo { nivel: 1, .. } if !b.latex.is_empty() => Estilo::do_latex(1, &b.latex),
            _ => None,
        });
        let serifa = if secao.is_some() { self.doc.blocos.iter().find_map(|b| match &b.conteudo { Conteudo::Paragrafo { texto } => texto.first().map(|t| t.estilo & crate::modelo::SERIFA), _ => None }).unwrap_or(0) } else { 0 };
        let marcas = marcadores(&self.doc.blocos, &self.doc.estilos.listas);
        let mut layouts = Vec::with_capacity(self.doc.blocos.len());
        for (i, (b, numero)) in self.doc.blocos.iter().zip(&self.numeros).enumerate() {
            // Logo depois de um título ("Referências" escrito pela pessoa), a lista não repete o dela.
            let depois_de_titulo = i > 0 && matches!(self.doc.blocos[i - 1].conteudo, Conteudo::Titulo { .. });
            let lay = match &b.conteudo {
                Conteudo::Sumario { niveis, sem_numeros, sem_pontilhado, lista: 0, latex } => Arc::new(sumario(&entradas, *niveis, (!*sem_numeros, !*sem_pontilhado, *latex), limite, &self.rotulo_sumario, secao.as_ref())),
                Conteudo::Sumario { sem_numeros, sem_pontilhado, lista, latex, .. } => {
                    let k = usize::from(*lista).min(2);
                    Arc::new(sumario(&self.entradas_da_lista(*lista), 1, (!*sem_numeros, !*sem_pontilhado, *latex), limite, &self.rotulos_lista[k - 1], secao.as_ref()))
                }
                Conteudo::Bibliografia { todas } => {
                    let titulo = if depois_de_titulo { "" } else { self.rotulo_referencias.as_str() };
                    let mut paragrafos = self.paragrafos_da_bibliografia(*todas, b.latex.strip_prefix("keyword="));
                    for t in paragrafos.iter_mut().flat_map(|p| &mut p.texto) {
                        t.estilo |= serifa;
                    }
                    Arc::new(lista_gerada(titulo, secao.as_ref(), &paragrafos, limite))
                }
                Conteudo::IndiceRemissivo => Arc::new(lista_gerada(&self.rotulo_indice, secao.as_ref(), &self.paragrafos_do_indice(), limite)),
                c => {
                    let estilo = self.doc.estilos.resolvido_do_bloco(b).or_else(|| match c { Conteudo::Titulo { nivel, .. } if !b.latex.is_empty() => Estilo::do_latex(*nivel, &b.latex), _ => None });
                    let estilo = estilo.as_ref();
                    // O item com rótulo de texto (o do LaTeX; o da lista do Word mais comprido que o vão do marcador, "Passo 1:") se compõe pelo rótulo (a largura dele empurra o texto).
                    let rotulo = marcas[i].clone().filter(|m| matches!(c, Conteudo::Item { .. }) && (b.latex.contains('|') || b.atrib.mais.lista != 0 && m.chars().count() > 4));
                    let assinatura: Assinatura = (rotulo.clone().or_else(|| numero.clone()), estilo.cloned());
                    match self.cache.get(&b.id) {
                        Some((a, lay)) if *a == assinatura => Arc::clone(lay),
                        antigo => {
                            if antigo.is_some() {
                                self.recompostos.push(b.id);
                            }
                            let lay = Arc::new(match c {
                                Conteudo::Tabela { linhas, .. } if !b.latex.is_empty() => {
                                    let mut t = crate::layout::tabela_latex(linhas, &b.latex, limite, &b.mesclagens, &b.atrib);
                                    // Como no LaTeX, a tabela que cabe numa página não se parte; a maior quebra linha a linha.
                                    let cfg = self.doc.pagina;
                                    if t.linhas.iter().map(|l| l.altura).sum::<f32>() <= cfg.altura - cfg.margem_sup - cfg.margem_inf {
                                        t.minimo_antes = t.linhas.len();
                                    }
                                    t
                                }
                                Conteudo::Item { nivel, texto, .. } if rotulo.is_some() => {
                                    let negrito = if b.latex.starts_with('b') { crate::modelo::NEGRITO } else { 0 };
                                    let a_direita = b.latex.split_once('|').is_some_and(|(e, _)| e.contains('d'));
                                    crate::layout::item_com_rotulo(*nivel, texto, limite, &b.atrib, rotulo.as_deref().unwrap_or(""), negrito, a_direita)
                                }
                                // O espaço de dentro da caixa (`topo=9`, ver `caixa_de_blocos` do leitor do LaTeX).
                                Conteudo::Paragrafo { .. } if b.latex.starts_with("topo=") => {
                                    let mut l = crate::layout::compor_em_mesclada(c, &b.atrib, limite, &Entorno { numero: numero.as_deref(), estilo }, &b.mesclagens);
                                    crate::layout::acolchoar_em_cima(&mut l, b.latex[5..].parse().unwrap_or(0.0));
                                    l
                                }
                                Conteudo::Codigo { linguagem, codigo } if !b.latex.is_empty() => crate::layout::codigo_latex(linguagem, codigo, &b.latex, limite, &b.atrib),
                                // O `quote` do LaTeX: texto normal, recuado dos dois lados (o `b.latex` diz quanto).
                                Conteudo::Citacao { texto } if !b.latex.is_empty() => {
                                    let recuo = b.latex.parse::<f32>().unwrap_or(25.0);
                                    let a = Atrib { recuo_esq: b.atrib.recuo_esq + recuo, recuo_dir: b.atrib.recuo_dir + recuo, ..b.atrib.clone() };
                                    crate::layout::compor_em(&Conteudo::Paragrafo { texto: texto.clone() }, &a, limite, &Entorno { numero: None, estilo: None })
                                }
                                _ => crate::layout::compor_em_tabela(c, &b.atrib, limite, &Entorno { numero: numero.as_deref(), estilo }, &b.mesclagens, &b.tabela),
                            });
                            self.cache.insert(b.id, (assinatura, Arc::clone(&lay)));
                            lay
                        }
                    }
                }
            };
            // A legenda de figura em cima da imagem (ABNT) não fica no pé da página sem ela.
            let imagem_depois = matches!(self.doc.blocos.get(i + 1).map(|p| &p.conteudo), Some(Conteudo::Imagem { .. }));
            // E a imagem não fica no pé da página sem a legenda dela embaixo.
            let legenda_depois = self.doc.blocos.get(i + 1).is_some_and(|p| p.atrib.legenda == 1 && matches!(p.conteudo, Conteudo::Paragrafo { .. }));
            let junto = (b.atrib.legenda == 1 && imagem_depois) || (matches!(b.conteudo, Conteudo::Imagem { .. }) && legenda_depois);
            let lay = if junto && !lay.manter_com_proximo {
                Arc::new(crate::layout::LayoutBloco { manter_com_proximo: true, ..(*lay).clone() })
            } else {
                lay
            };
            layouts.push(lay);
        }
        self.paginas = paginar(&layouts, &marcas, self.doc.pagina.altura_util());
        alinhar_na_vertical(&mut self.paginas, self.doc.pagina.altura_util(), self.doc.extras.vertical);
        self.layouts = layouts;
        self.indices = self.doc.blocos.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        self.titulo_na_pagina.clear();
        self.primeira_pagina_do_bloco = vec![0; self.doc.blocos.len()];
        let mut visto = vec![false; self.doc.blocos.len()];
        for (n, p) in self.paginas.iter().enumerate() {
            for f in &p.fatias {
                if !visto[f.bloco] {
                    visto[f.bloco] = true;
                    self.primeira_pagina_do_bloco[f.bloco] = n;
                }
                if f.de == 0 {
                    self.titulo_na_pagina.entry(self.doc.blocos[f.bloco].id).or_insert(n);
                }
            }
        }
    }

    /// Os títulos, com o número na frente quando a numeração está ligada.
    pub(crate) fn entradas_do_sumario(&self) -> Vec<EntradaSumario> {
        self.doc
            .blocos
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                texto_do_titulo(b).map(|(nivel, texto)| {
                    let texto = match self.numeros.get(i).and_then(Option::as_deref) {
                        Some(n) => format!("{n}  {texto}"),
                        None => texto,
                    };
                    EntradaSumario { id: b.id, nivel, texto }
                })
            })
            .collect()
    }

    /// As legendas do tipo `lista` (1 figura, 2 tabela), com o número, para a lista de figuras ou de tabelas.
    fn entradas_da_lista(&self, lista: u8) -> Vec<EntradaSumario> {
        self.doc
            .blocos
            .iter()
            .enumerate()
            .filter(|(_, b)| b.atrib.legenda == lista && matches!(b.conteudo, Conteudo::Paragrafo { .. }))
            .map(|(i, b)| {
                let texto = trechos_de(&b.conteudo).map(|t| texto_puro(t)).unwrap_or_default();
                let numero = self.numeros.get(i).cloned().flatten().unwrap_or_default();
                // A legenda do LaTeX entra como no `\listoffigures`: só o número numa coluna ("1  Título").
                if !b.latex.is_empty() {
                    let n = numero.trim().trim_end_matches([':', '.', '–']).trim().rsplit(' ').next().unwrap_or("").to_string();
                    return EntradaSumario { id: b.id, nivel: 0, texto: format!("{n}  {texto}").trim().to_string() };
                }
                EntradaSumario { id: b.id, nivel: 0, texto: format!("{numero} {texto}").trim().to_string() }
            })
            .collect()
    }

    /// A posição do bloco na lista.
    pub(crate) fn idx(&self, id: u64) -> Option<usize> {
        self.indices.get(&id).copied()
    }

    /// A página (0, 1…) onde o bloco começa.
    pub fn pagina_do_bloco(&self, id: u64) -> Option<usize> {
        self.titulo_na_pagina.get(&id).copied()
    }

    /// O número que a página `p` (0, 1…) mostra: conta a partir de `Extras::pagina_inicial`.
    pub fn numero_da_pagina(&self, p: usize) -> usize {
        p + self.doc.extras.pagina_inicial as usize
    }

    /// Quantas páginas.
    pub fn total_de_paginas(&self) -> usize {
        self.paginas.len()
    }

    /// A versão do documento (sobe a cada edição).
    pub fn versao(&self) -> u64 {
        self.versao
    }

    /// Declara que o estado atual está no disco.
    pub fn marcar_gravado(&mut self) {
        self.versao_gravada = self.versao;
    }

    fn aplicar(&mut self, s: Splice) -> Splice {
        let fim = (s.pos + s.quantos).min(self.doc.blocos.len());
        for b in &self.doc.blocos[s.pos..fim] {
            self.cache.remove(&b.id);
        }
        let inseridos = s.dentro.len();
        let removidos: Vec<Bloco> = self.doc.blocos.splice(s.pos..fim, s.dentro).collect();
        Splice { pos: s.pos, quantos: inseridos, dentro: removidos }
    }

    fn fim_da_edicao(&mut self, mut afetados: Vec<u64>, pos: usize) -> Mudanca {
        self.versao += 1;
        let titulos_antes = self.paginas_dos_titulos();
        self.recompostos.clear();
        self.repaginar();
        afetados.append(&mut self.recompostos);
        let mut vistos = std::collections::HashSet::new();
        afetados.retain(|id| vistos.insert(*id));
        self.validar_cursor();
        // O sumário só muda se um título foi mexido ou mudou de página.
        let sumario_mudou = titulos_antes != self.paginas_dos_titulos() || afetados.iter().any(|id| self.idx(*id).is_some_and(|i| entra_em_sumario(&self.doc.blocos[i])));
        let primeira_pagina = self.primeira_pagina_mudada(&afetados, pos, sumario_mudou);
        Mudanca { versao: self.versao, paginas: self.paginas.len(), afetados, primeira_pagina }
    }

    /// A página de cada título (para saber se o sumário muda).
    fn paginas_dos_titulos(&self) -> Vec<(u64, Option<usize>)> {
        self.doc.blocos.iter().filter(|b| entra_em_sumario(b)).map(|b| (b.id, self.pagina_do_bloco(b.id))).collect()
    }

    /// Uma página antes da primeira tocada (um título que ia junto com o texto pode ter voltado), e a do sumário.
    fn primeira_pagina_mudada(&self, afetados: &[u64], pos: usize, sumario_mudou: bool) -> usize {
        let das_edicoes = afetados.iter().filter_map(|id| self.pagina_do_bloco(*id));
        let do_sumario = self.doc.blocos.iter().filter(|b| sumario_mudou && matches!(b.conteudo, Conteudo::Sumario { .. })).filter_map(|b| self.pagina_do_bloco(b.id));
        let da_posicao = self.doc.blocos.get(pos.min(self.doc.blocos.len().saturating_sub(1))).and_then(|b| self.pagina_do_bloco(b.id));
        das_edicoes.chain(do_sumario).chain(da_posicao).min().unwrap_or(0).saturating_sub(1)
    }

    /// Depois de uma edição ou de desfazer: o cursor não pode apontar para um bloco que sumiu, nem para além do texto.
    fn validar_cursor(&mut self) {
        let primeiro = self.doc.blocos[0].id;
        for p in [&mut self.cursor.ancora, &mut self.cursor.foco] {
            let Some(i) = self.indices.get(&p.bloco).copied() else {
                *p = Pos { bloco: primeiro, celula: primeiro_alvo(&self.doc.blocos[0].conteudo).flatten(), desl: 0 };
                continue;
            };
            let c = &self.doc.blocos[i].conteudo;
            if eh_atomico(c) {
                *p = Pos { bloco: p.bloco, celula: None, desl: 0 };
            } else {
                p.desl = p.desl.min(crate::alvo::tamanho_alvo(c, p.celula));
            }
        }
    }

    /// Uma mudança que não toca bloco nenhum (cabeçalho, plano de fundo): nova versão, tudo repinta.
    pub(crate) fn fim_da_edicao_sem_blocos(&mut self) -> Mudanca {
        let m = self.fim_da_edicao(Vec::new(), 0);
        Mudanca { primeira_pagina: 0, afetados: self.doc.blocos.first().map(|b| vec![b.id]).unwrap_or_default(), ..m }
    }

    /// Só a tela mudou (marcas de formatação): nova versão do quadro sem sujar o documento.
    pub(crate) fn nova_versao_de_tela(&mut self) -> Mudanca {
        let limpo = self.versao == self.versao_gravada;
        self.versao += 1;
        if limpo {
            self.versao_gravada = self.versao;
        }
        Mudanca { versao: self.versao, paginas: self.paginas.len(), afetados: self.doc.blocos.first().map(|b| vec![b.id]).unwrap_or_default(), primeira_pagina: 0 }
    }

    pub(crate) fn sem_mudanca(&self) -> Mudanca {
        Mudanca { versao: self.versao, paginas: self.paginas.len(), afetados: Vec::new(), primeira_pagina: self.paginas.len() }
    }

    /// Aplica uma edição, põe o cursor onde ela o deixa e guarda o passo de desfazer. Com `continuo`
    /// (o bloco e o tipo de edição), repetir a mesma coisa no mesmo bloco vira o mesmo passo.
    pub(crate) fn transacao(&mut self, s: Splice, cursor: Cursor, continuo: Option<(u64, u8)>) -> Mudanca {
        if self.bloqueado() && !self.comentando {
            return self.sem_mudanca();
        }
        let (s, cursor) = if self.rastreando() { self.como_sugestao(s, cursor) } else { (s, cursor) };
        let mut afetados: Vec<u64> = s.dentro.iter().map(|b| b.id).collect();
        afetados.extend(self.doc.blocos[s.pos..(s.pos + s.quantos).min(self.doc.blocos.len())].iter().map(|b| b.id));
        let pos = s.pos;
        let antes = self.cursor;
        let inverso = self.aplicar(s);
        let funde = continuo.is_some() && self.digitando == continuo && !self.desfazer.is_empty();
        if !funde {
            self.desfazer.push(Passo { s: inverso, cursor: antes, estilos: None });
            if self.desfazer.len() > MAXIMO_DESFAZER {
                self.desfazer.remove(0);
            }
        }
        self.refazer.clear();
        self.cursor = cursor;
        self.digitando = continuo;
        self.fim_da_edicao(afetados, pos)
    }

    /// Desfaz a última edição (e devolve o cursor para onde estava).
    pub fn desfazer(&mut self) -> Option<Mudanca> {
        if self.bloqueado() {
            return None;
        }
        let p = self.desfazer.pop()?;
        Some(self.voltar(p, true))
    }

    /// Refaz o que foi desfeito.
    pub fn refazer(&mut self) -> Option<Mudanca> {
        if self.bloqueado() {
            return None;
        }
        let p = self.refazer.pop()?;
        Some(self.voltar(p, false))
    }

    fn voltar(&mut self, p: Passo, desfazendo: bool) -> Mudanca {
        let pos = p.s.pos;
        let afetados: Vec<u64> = p.s.dentro.iter().map(|b| b.id).chain(self.doc.blocos[pos..(pos + p.s.quantos).min(self.doc.blocos.len())].iter().map(|b| b.id)).collect();
        let cursor_antes = self.cursor;
        let inverso = self.aplicar(p.s);
        let estilos = p.estilos.map(|e| std::mem::replace(&mut self.doc.estilos, e));
        let passo = Passo { s: inverso, cursor: cursor_antes, estilos };
        if desfazendo {
            self.refazer.push(passo);
        } else {
            self.desfazer.push(passo);
        }
        self.cursor = p.cursor;
        self.digitando = None;
        self.fim_da_edicao(afetados, pos)
    }

    /// Uma edição que muda os estilos com nome ou a numeração (e talvez blocos): `estilos_antes` volta no desfazer.
    pub(crate) fn transacao_de_estilos(&mut self, s: Splice, cursor: Cursor, estilos_antes: Estilos) -> Mudanca {
        let m = self.transacao(s, cursor, None);
        if let Some(p) = self.desfazer.last_mut() {
            p.estilos = Some(estilos_antes);
        }
        m
    }

    /// As células da tabela em texto puro (o que o FeatherSheet recebe).
    pub fn tabela_celulas(&self, id: u64) -> Result<Vec<Vec<String>>, String> {
        let i = self.idx(id).ok_or("O bloco não existe mais.")?;
        match &self.doc.blocos[i].conteudo {
            Conteudo::Tabela { linhas, .. } => Ok(tabela_em_texto(linhas)),
            _ => Err("O bloco não é uma tabela.".to_string()),
        }
    }

    /// Troca as células da tabela (o que volta do FeatherSheet), mantendo o cabeçalho.
    pub fn tabela_substituir(&mut self, id: u64, celulas: &[Vec<String>]) -> Result<Mudanca, String> {
        let i = self.idx(id).ok_or("O bloco não existe mais.")?;
        let Conteudo::Tabela { cabecalho, .. } = self.doc.blocos[i].conteudo else {
            return Err("O bloco não é uma tabela.".to_string());
        };
        if !self.doc.blocos[i].mesclagens.is_empty() {
            return Err("Desmescle as células antes de substituir a tabela em texto puro.".into());
        }
        let conteudo = tabela_de_texto(celulas, cabecalho);
        let cursor = self.cursor;
        let mut bloco = self.doc.blocos[i].clone();
        bloco.conteudo = conteudo;
        Ok(self.transacao(Splice { pos: i, quantos: 1, dentro: vec![bloco] }, cursor, None))
    }

    /// Troca o papel (`a4` ou `carta`); a composição de todos os blocos é refeita.
    pub fn papel_definir(&mut self, nome: &str) -> Result<Mudanca, String> {
        let (l, a) = tamanho_do_papel(nome).ok_or_else(|| format!("Não conheço o papel \"{nome}\"."))?;
        let paisagem = self.doc.pagina.largura > self.doc.pagina.altura;
        let (l, a) = if paisagem { (a, l) } else { (l, a) };
        self.doc.pagina = ConfigPagina { largura: l, altura: a, ..self.doc.pagina };
        self.cache.clear();
        Ok(self.fim_da_edicao(Vec::new(), 0))
    }

    /// Um papel de medida própria (pontos). Recusa o que não deixa espaço para o texto com as margens de agora.
    pub fn papel_personalizado(&mut self, largura: f32, altura: f32) -> Result<Mudanca, String> {
        let p = self.doc.pagina;
        if !(largura.is_finite() && altura.is_finite()) || largura - p.margem_esq - p.margem_dir < 100.0 || altura - p.margem_sup - p.margem_inf < 100.0 {
            return Err("O papel é pequeno demais para as margens de agora.".to_string());
        }
        self.doc.pagina = ConfigPagina { largura, altura, ..p };
        self.cache.clear();
        Ok(self.fim_da_edicao(Vec::new(), 0))
    }

    /// Retrato ou paisagem: troca a largura pela altura do papel.
    pub fn orientacao_definir(&mut self, paisagem: bool) -> Result<Mudanca, String> {
        let p = self.doc.pagina;
        if (p.largura > p.altura) == paisagem {
            return Ok(self.sem_mudanca());
        }
        self.papel_personalizado(p.altura, p.largura)
    }

    /// Define as margens (em pontos). Recusa margem negativa ou que deixe menos de 100 pt de área de texto.
    pub fn margens_definir(&mut self, esq: f32, dir: f32, sup: f32, inf: f32) -> Result<Mudanca, String> {
        const MINIMO: f32 = 100.0;
        let p = self.doc.pagina;
        if [esq, dir, sup, inf].iter().any(|m| !m.is_finite() || *m < 0.0) {
            return Err("A margem não pode ser negativa.".to_string());
        }
        if p.largura - esq - dir < MINIMO || p.altura - sup - inf < MINIMO {
            return Err("As margens deixariam o texto sem espaço.".to_string());
        }
        self.doc.pagina = ConfigPagina { margem_esq: esq, margem_dir: dir, margem_sup: sup, margem_inf: inf, ..p };
        self.cache.clear();
        Ok(self.fim_da_edicao(Vec::new(), 0))
    }

    /// Os títulos do documento com a página de cada um (o painel de estrutura).
    pub fn estrutura(&self) -> Vec<Titulo> {
        self.entradas_do_sumario().into_iter().map(|e| Titulo { pagina: self.pagina_do_bloco(e.id).unwrap_or(0), id: e.id, nivel: e.nivel, texto: e.texto }).collect()
    }

    /// Título e autor do documento.
    pub fn meta_definir(&mut self, titulo: &str, autor: &str) {
        self.doc.meta.titulo = titulo.to_string();
        self.doc.meta.autor = autor.to_string();
        self.versao += 1;
    }

    /// O resumo do estado, com a contagem de palavras.
    pub fn estado(&self) -> Estado {
        let (palavras, caracteres) = self.contar();
        Estado {
            versao: self.versao,
            paginas: self.paginas.len(),
            blocos: self.doc.blocos.len(),
            palavras,
            caracteres,
            pode_desfazer: !self.desfazer.is_empty(),
            pode_refazer: !self.refazer.is_empty(),
            sujo: self.versao != self.versao_gravada,
            titulo: self.doc.meta.titulo.clone(),
            largura: self.doc.pagina.largura,
            altura: self.doc.pagina.altura,
            margem_esq: self.doc.pagina.margem_esq,
            margem_dir: self.doc.pagina.margem_dir,
            margem_sup: self.doc.pagina.margem_sup,
            margem_inf: self.doc.pagina.margem_inf,
            papel: nome_do_papel(&self.doc.pagina).to_string(),
            paisagem: self.doc.pagina.largura > self.doc.pagina.altura,
            marcas: self.marcas,
            grade: self.grade,
            extras: self.doc.extras.clone(),
            estilos: self.doc.estilos.clone(),
            estilo_citacao: self.doc.bibliografia.estilo,
            modo_edicao: self.modo_edicao().to_string(),
            tem_revisoes: self.doc.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().any(|t| t.rev.is_some()),
        }
    }

    fn contar(&self) -> (usize, usize) {
        let (mut p, mut c) = (0, 0);
        let mut somar = |t: &str| {
            p += t.split_whitespace().count();
            c += t.chars().filter(|x| !x.is_whitespace()).count();
        };
        for b in &self.doc.blocos {
            if let Some(t) = trechos_de(&b.conteudo) {
                somar(&texto_puro(t));
            } else if let Conteudo::Tabela { linhas, .. } = &b.conteudo {
                linhas.iter().flatten().for_each(|cel| somar(&texto_puro(cel)));
            } else if let Conteudo::Imagem { legenda, .. } = &b.conteudo {
                somar(&texto_puro(legenda));
            }
        }
        (p, c)
    }
}

/// O bloco aparece num sumário ou numa lista (título ou legenda): mexer nele repinta o sumário.
fn entra_em_sumario(b: &Bloco) -> bool {
    matches!(b.conteudo, Conteudo::Titulo { .. }) || b.atrib.legenda != 0
}

/// Largura e altura (retrato, em pontos) de um papel pelo nome.
fn tamanho_do_papel(nome: &str) -> Option<(f32, f32)> {
    Some(match nome {
        "a4" => (595.28, 841.89),
        "carta" => (612.0, 792.0),
        "oficio" => (612.0, 1008.0),
        "a3" => (841.89, 1190.55),
        "a5" => (419.53, 595.28),
        _ => return None,
    })
}

/// O nome do papel que tem estas medidas (em qualquer orientação), ou `personalizado`.
fn nome_do_papel(p: &ConfigPagina) -> &'static str {
    let (l, a) = (p.largura.min(p.altura), p.largura.max(p.altura));
    ["a4", "carta", "oficio", "a3", "a5"]
        .into_iter()
        .find(|n| tamanho_do_papel(n).is_some_and(|(pl, pa)| (pl - l).abs() < 0.5 && (pa - a).abs() < 0.5))
        .unwrap_or("personalizado")
}
