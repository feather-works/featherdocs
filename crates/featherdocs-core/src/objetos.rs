//! Os objetos presos ao texto (`modelo::Marca`): notas de rodapé e de fim, citações, entradas de
//! índice e campos. Cada um ocupa **um caractere** do texto; o que ele mostra (o rótulo: "1",
//! "(SILVA, 2023)", o nome do autor) o motor refaz a cada mudança, antes de compor.
//!
//! As notas guardam o texto dentro do próprio objeto: desfazer, copiar e colar levam a nota junto,
//! e apagar a chamada apaga a nota — sem lista paralela para manter em dia.

use serde::Serialize;

use crate::alvo::{com_texto_ref, eh_texto_cru, texto_alvo};
use crate::cursor::{Cursor, Pos};
use crate::inline::{analisar, serializar};
use crate::modelo::{Marca, Trecho};
use crate::paginacao::{romano, RefNota, ALTURA_DO_TITULO_DAS_NOTAS, ESPACO_DAS_NOTAS};
use crate::sessao::{Mudanca, Sessao, Splice};
use crate::texto::{inserir_objeto, objeto_em, trocar_objeto};

/// O número de uma nota no formato escolhido: 0 = 1, 2, 3 · 1 = a, b, c · 2 = i, ii, iii · 3 = *, †, ‡.
pub fn numero_de_nota(n: u32, formato: u8) -> String {
    match formato {
        1 => {
            let mut s = Vec::new();
            let mut k = n;
            while k > 0 {
                k -= 1;
                s.push((b'a' + (k % 26) as u8) as char);
                k /= 26;
            }
            s.iter().rev().collect()
        }
        2 => romano(n).to_lowercase(),
        3 => {
            const SIMBOLOS: [&str; 4] = ["*", "†", "‡", "§"];
            let k = (n.max(1) - 1) as usize;
            SIMBOLOS[k % 4].repeat(k / 4 + 1)
        }
        _ => n.to_string(),
    }
}

/// O que vem da interface e aparece no documento (vazio = o padrão em português).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct ContextoExterno {
    #[serde(default)]
    pub hoje: String,
    #[serde(default)]
    pub arquivo: String,
    #[serde(default)]
    pub notas: String,
    #[serde(default)]
    pub referencias: String,
    #[serde(default)]
    pub indice: String,
    /// Quem edita (autor das sugestões e dos comentários), das Configurações.
    #[serde(default)]
    pub autor: String,
    /// O "autor" das marcas de Revisão → Comparar ("Comparação").
    #[serde(default)]
    pub comparacao: String,
}

/// O que a caixa de nota mostra para editar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InfoNota {
    /// O texto da nota como fonte em linha (`**negrito**`, `*itálico*`…).
    pub fonte: String,
    pub fim: bool,
    /// O número que ela mostra agora.
    pub numero: String,
}

/// Onde uma nota está desenhada no pé da página ou na lista do fim (em pontos, do topo da página).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaixaDeNota {
    pub y: f32,
    pub h: f32,
    pub nota: RefNota,
}

/// O que um campo mostra, fora os de página (que só se sabem depois de paginar).
fn valor_do_campo(campo: &str, meta: &crate::modelo::Meta, hoje: &str, arquivo: &str) -> String {
    let v = match campo {
        "autor" => meta.autor.as_str(),
        "titulo" => meta.titulo.as_str(),
        "data" => hoje,
        "arquivo" => arquivo,
        "pagina" | "paginas" => "1",
        _ => "?",
    };
    if v.is_empty() {
        format!("[{campo}]")
    } else {
        v.to_string()
    }
}

impl Sessao {
    /// Refaz o rótulo de cada objeto; o bloco em que algum mudou é recomposto (e repintado).
    pub(crate) fn rotular(&mut self) {
        let (fmt_rodape, fmt_fim) = (self.doc.extras.formato_notas, self.doc.extras.formato_notas_fim);
        let (mut rodape, mut fim) = (0u32, 0u32);
        let meta = self.doc.meta.clone();
        let bib = self.doc.bibliografia.clone();
        let numeros = crate::citacoes::numeros_das_fontes(bib.estilo, &crate::citacoes::chaves_citadas(&self.doc.blocos));
        for b in &mut self.doc.blocos {
            let mut mudou = false;
            for t in b.conteudo.textos_mut() {
                for tr in t.iter_mut() {
                    let Some(m) = tr.marca.as_deref() else { continue };
                    let r = match m {
                        Marca::Nota { fim: true, .. } => {
                            fim += 1;
                            numero_de_nota(fim, fmt_fim)
                        }
                        Marca::Nota { .. } => {
                            rodape += 1;
                            numero_de_nota(rodape, fmt_rodape)
                        }
                        Marca::Citacao { chaves, pagina } => crate::citacoes::rotulo_da_citacao(&bib, &numeros, chaves, pagina),
                        Marca::Indice { .. } | Marca::Comentario { .. } | Marca::Imagem { .. } | Marca::Controle { .. } => String::new(),
                        Marca::ObjetoWord { resumo, .. } => format!("[{resumo} do Word]"),
                        Marca::Campo { campo } if campo == "pagina" || campo == "paginas" => continue,
                        Marca::Campo { campo } => valor_do_campo(campo, &meta, &self.hoje, &self.arquivo),
                    };
                    if tr.rotulo != r {
                        tr.rotulo = r;
                        mudou = true;
                    }
                }
            }
            if mudou {
                self.cache.remove(&b.id);
                self.recompostos.push(b.id);
            }
        }
    }

    /// Depois de paginar: o campo `pagina` mostra a página onde está, e `paginas` o total.
    /// `true` se algum mudou (então é preciso compor de novo).
    pub(crate) fn rotular_paginas(&mut self) -> bool {
        let total = self.numero_da_pagina(self.paginas.len().saturating_sub(1)).to_string();
        let mut trocas = Vec::new();
        for (i, b) in self.doc.blocos.iter().enumerate() {
            for (k, t) in b.conteudo.textos().into_iter().enumerate() {
                let mut pos = 0;
                for (j, tr) in t.iter().enumerate() {
                    if let Some(Marca::Campo { campo }) = tr.marca.as_deref().filter(|m| matches!(m, Marca::Campo { campo } if campo == "pagina" || campo == "paginas")) {
                        let r = if campo == "paginas" { total.clone() } else { self.pagina_do_caractere(i, pos).to_string() };
                        if tr.rotulo != r {
                            trocas.push((i, k, j, r));
                        }
                    }
                    pos += tr.texto.chars().count();
                }
            }
        }
        for (i, k, j, r) in &trocas {
            let b = &mut self.doc.blocos[*i];
            if let Some(t) = b.conteudo.textos_mut().into_iter().nth(*k) {
                t[*j].rotulo.clone_from(r);
            }
            self.cache.remove(&b.id);
            self.recompostos.push(b.id);
        }
        !trocas.is_empty()
    }

    /// A página (1, 2…) onde está o caractere `desl` do texto corrido do bloco `i`.
    pub(crate) fn pagina_do_caractere(&self, i: usize, desl: usize) -> usize {
        let li = if i < self.layouts.len() { self.linha_do_desl(i, desl) } else { 0 };
        self.fatias_do_bloco(i).into_iter().find(|(_, f)| li >= f.de && li < f.ate).map_or(self.numero_da_pagina(0), |(p, _)| self.numero_da_pagina(p))
    }

    /// Formatação das notas: o formato do número das de rodapé e das de fim.
    pub fn notas_formato(&mut self, rodape: u8, fim: u8) -> Result<Mudanca, String> {
        if rodape > 3 || fim > 3 {
            return Err("Formato de nota desconhecido.".to_string());
        }
        self.doc.extras.formato_notas = rodape;
        self.doc.extras.formato_notas_fim = fim;
        Ok(self.fim_da_edicao_sem_blocos())
    }

    /// Prende um objeto no cursor (a seleção, se houver, fica: o objeto entra no fim dela).
    pub fn objeto_inserir(&mut self, marca: Marca) -> Result<Mudanca, String> {
        let f = self.cursor.foco;
        let i = self.idx(f.bloco).ok_or("O cursor está fora do texto.")?;
        let mut b = self.doc.blocos[i].clone();
        if eh_texto_cru(&b.conteudo) {
            return Err("Não dá para pôr isto dentro de código ou de fórmula.".to_string());
        }
        com_texto_ref(&mut b.conteudo, f.celula, |x| inserir_objeto(x, f.desl, marca)).ok_or("Aqui não tem texto para receber isto.")?;
        let cursor = Cursor::em(Pos { desl: f.desl + 1, ..f });
        Ok(self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None))
    }

    /// Referências → Inserir nota de rodapé (ou de fim): a chamada no cursor e o texto dela.
    pub fn nota_inserir(&mut self, fim: bool, fonte: &str) -> Result<Mudanca, String> {
        self.objeto_inserir(Marca::Nota { fim, texto: analisar(fonte) })
    }

    /// O objeto que ocupa o caractere de `p`.
    pub fn objeto_em(&self, p: &Pos) -> Option<Marca> {
        let i = self.idx(p.bloco)?;
        let t = texto_alvo(&self.doc.blocos[i].conteudo, p.celula)?;
        objeto_em(&t, p.desl).cloned()
    }

    /// A nota cuja chamada está em `p`, para a caixa de edição.
    pub fn nota_info(&self, p: &Pos) -> Result<InfoNota, String> {
        let i = self.idx(p.bloco).ok_or("A nota não existe mais.")?;
        let t = texto_alvo(&self.doc.blocos[i].conteudo, p.celula).ok_or("A nota não existe mais.")?;
        let rotulo = crate::texto::fatia(&t, p.desl, p.desl + 1).into_iter().next().map(|x| x.rotulo).unwrap_or_default();
        match objeto_em(&t, p.desl) {
            Some(Marca::Nota { fim, texto }) => Ok(InfoNota { fonte: serializar(texto), fim: *fim, numero: rotulo }),
            _ => Err("Aqui não há nota.".to_string()),
        }
    }

    /// Troca o texto da nota em `p` (e se ela é de rodapé ou de fim).
    pub fn nota_editar(&mut self, p: &Pos, fonte: &str, fim: bool) -> Result<Mudanca, String> {
        self.nota_info(p)?;
        self.objeto_trocar(p, Marca::Nota { fim, texto: analisar(fonte) })
    }

    /// Troca o objeto em `p` por outro (um passo de desfazer).
    pub(crate) fn objeto_trocar(&mut self, p: &Pos, marca: Marca) -> Result<Mudanca, String> {
        let i = self.idx(p.bloco).ok_or("O objeto não existe mais.")?;
        let mut b = self.doc.blocos[i].clone();
        let ok = com_texto_ref(&mut b.conteudo, p.celula, |x| trocar_objeto(x, p.desl, marca)).unwrap_or(false);
        if !ok {
            return Err("Aqui não há objeto.".to_string());
        }
        let cursor = self.cursor;
        Ok(self.transacao(Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None))
    }

    /// Onde a chamada de uma nota composta está no texto.
    pub(crate) fn pos_da_nota(&self, r: RefNota) -> Option<Pos> {
        let n = self.layouts.get(r.bloco)?.linhas.get(r.linha)?.notas.get(r.nota)?;
        let id = self.doc.blocos.get(r.bloco)?.id;
        Some(Pos { bloco: id, celula: n.coluna.map(|c| (r.linha as u32, u32::from(c))), desl: n.desl })
    }

    pub(crate) fn nota_composta(&self, r: RefNota) -> Option<&crate::layout::NotaComposta> {
        self.layouts.get(r.bloco)?.linhas.get(r.linha)?.notas.get(r.nota)
    }

    /// As notas de rodapé da página empilhadas acima da margem de baixo, e as de fim onde a paginação as pôs.
    pub(crate) fn caixas_das_notas(&self, pagina: usize) -> Vec<CaixaDeNota> {
        let Some(p) = self.paginas.get(pagina) else { return Vec::new() };
        let cfg = self.doc.pagina;
        let mut saida = Vec::new();
        let total: f32 = p.notas.iter().filter_map(|r| self.nota_composta(*r)).map(crate::layout::NotaComposta::altura).sum();
        let mut y = cfg.altura - cfg.margem_inf - total;
        for r in &p.notas {
            let Some(n) = self.nota_composta(*r) else { continue };
            saida.push(CaixaDeNota { y, h: n.altura(), nota: *r });
            y += n.altura();
        }
        for (topo, r) in &p.notas_fim {
            if let Some(n) = r.and_then(|r| self.nota_composta(r)) {
                saida.push(CaixaDeNota { y: cfg.margem_sup + topo, h: n.altura(), nota: r.unwrap_or(RefNota { bloco: 0, linha: 0, nota: 0 }) });
            }
        }
        saida
    }

    /// O topo do traço das notas de rodapé da página (para o quadro).
    pub(crate) fn topo_das_notas(&self, pagina: usize) -> Option<f32> {
        let caixas = self.caixas_das_notas(pagina);
        let p = self.paginas.get(pagina)?;
        (!p.notas.is_empty()).then(|| caixas.first().map_or(0.0, |c| c.y) - ESPACO_DAS_NOTAS + 4.0)
    }

    /// A nota sob o ponto (`x`, `y`) da página: no pé, na lista do fim ou a própria chamada no texto.
    pub fn nota_no_ponto(&self, pagina: usize, x: f32, y: f32) -> Option<Pos> {
        if let Some(c) = self.caixas_das_notas(pagina).into_iter().find(|c| y >= c.y && y < c.y + c.h) {
            return self.pos_da_nota(c.nota);
        }
        let p = self.pos_na_pagina(pagina, x, y)?;
        let antes = Pos { desl: p.desl.saturating_sub(1), ..p };
        [p, antes].into_iter().find(|q| matches!(self.objeto_em(q), Some(Marca::Nota { .. })))
    }

    /// O objeto sob o ponto (`x`, `y`) da página — a chamada no texto, ou a nota no pé ou no fim — e
    /// onde ele está (para o duplo clique abrir a caixa certa).
    pub fn objeto_no_ponto(&self, pagina: usize, x: f32, y: f32) -> Option<(Pos, Marca)> {
        if let Some(c) = self.caixas_das_notas(pagina).into_iter().find(|c| y >= c.y && y < c.y + c.h) {
            let p = self.pos_da_nota(c.nota)?;
            return self.objeto_em(&p).map(|m| (p, m));
        }
        let p = self.pos_na_pagina(pagina, x, y)?;
        let antes = Pos { desl: p.desl.saturating_sub(1), ..p };
        [p, antes].into_iter().find_map(|q| self.objeto_em(&q).map(|m| (q, m)))
    }

    /// O título das notas de fim (vem da língua da interface; o padrão é "Notas").
    pub(crate) fn titulo_das_notas(&self) -> &str {
        &self.rotulo_notas
    }

    /// O que vem de fora do documento e aparece nele: a data de hoje (campo `data`), o nome do
    /// arquivo (campo `arquivo`) e os títulos das notas de fim, das referências e do índice. Só a
    /// tela muda (o documento não fica sujo).
    pub fn contexto_definir(&mut self, c: &ContextoExterno) -> Mudanca {
        let ou = |v: &str, padrao: &str| if v.is_empty() { padrao.to_string() } else { v.to_string() };
        let novo = (c.hoje.clone(), c.arquivo.clone(), ou(&c.notas, "Notas"), ou(&c.referencias, "Referências"), ou(&c.indice, "Índice"));
        let atual = (self.hoje.clone(), self.arquivo.clone(), self.rotulo_notas.clone(), self.rotulo_referencias.clone(), self.rotulo_indice.clone());
        if novo == atual {
            self.autor.clone_from(&c.autor);
            return self.sem_mudanca();
        }
        (self.hoje, self.arquivo, self.rotulo_notas, self.rotulo_referencias, self.rotulo_indice) = novo;
        self.autor.clone_from(&c.autor);
        self.repaginar();
        self.nova_versao_de_tela()
    }
}

/// A altura do título das notas de fim (reexportada para o quadro).
pub(crate) const TITULO_DAS_NOTAS: f32 = ALTURA_DO_TITULO_DAS_NOTAS;

/// O texto de uma nota como trechos (para os leitores de arquivo).
pub fn nota(fim: bool, texto: Vec<Trecho>) -> Trecho {
    Trecho::objeto(Marca::Nota { fim, texto })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{Conteudo, Documento};

    fn sessao(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn ir(s: &mut Sessao, bloco: usize, desl: usize) {
        let id = s.documento().blocos[bloco].id;
        s.cursor = Cursor::em(Pos { bloco: id, celula: None, desl });
    }

    #[test]
    fn numeros_de_nota_nos_quatro_formatos() {
        assert_eq!(numero_de_nota(3, 0), "3");
        assert_eq!(numero_de_nota(28, 1), "ab");
        assert_eq!(numero_de_nota(4, 2), "iv");
        assert_eq!(numero_de_nota(2, 3), "†");
        assert_eq!(numero_de_nota(5, 3), "**");
    }

    #[test]
    fn notas_se_numeram_na_ordem_e_desfazer_tira_a_nota() {
        let mut s = sessao(&["primeiro", "segundo"]);
        ir(&mut s, 1, 7);
        s.nota_inserir(false, "Nota do **segundo**.").unwrap();
        ir(&mut s, 0, 8);
        s.nota_inserir(false, "Nota do primeiro.").unwrap();
        let rotulos: Vec<String> = s.documento().blocos.iter().flat_map(|b| b.conteudo.textos().into_iter().flatten().filter(|t| t.marca.is_some()).map(|t| t.rotulo.clone()).collect::<Vec<_>>()).collect();
        assert_eq!(rotulos, ["1", "2"], "a nota de cima passa a ser a 1");
        let id = s.documento().blocos[1].id;
        let info = s.nota_info(&Pos { bloco: id, celula: None, desl: 7 }).unwrap();
        assert_eq!((info.fonte.as_str(), info.numero.as_str()), ("Nota do **segundo**.", "2"));
        assert_eq!(s.paginas[0].notas.len(), 2, "as duas no pé da primeira página");
        s.desfazer();
        assert_eq!(s.paginas[0].notas.len(), 1);
    }

    #[test]
    fn editar_converte_em_nota_de_fim() {
        let mut s = sessao(&["texto"]);
        ir(&mut s, 0, 5);
        s.nota_inserir(false, "a").unwrap();
        let id = s.documento().blocos[0].id;
        let p = Pos { bloco: id, celula: None, desl: 5 };
        s.nota_editar(&p, "b", true).unwrap();
        assert!(s.paginas[0].notas.is_empty());
        assert_eq!(s.paginas.last().unwrap().notas_fim.len(), 2);
        assert_eq!(s.nota_info(&p).unwrap().numero, "i");
        assert!(s.nota_editar(&Pos { desl: 0, ..p }, "x", false).is_err());
    }

    #[test]
    fn cursor_passa_pela_chamada_como_um_caractere() {
        let mut s = sessao(&["ab"]);
        ir(&mut s, 0, 1);
        s.nota_inserir(false, "n").unwrap();
        assert_eq!(s.cursor.foco.desl, 2);
        let g1 = s.geometria(&Pos { desl: 1, ..s.cursor.foco }).unwrap();
        let g2 = s.geometria(&s.cursor.foco).unwrap();
        assert!(g2.x > g1.x, "a chamada tem largura");
        let p = s.nota_no_ponto(0, g1.x + 0.5, g1.y + g1.h / 2.0);
        assert_eq!(p.map(|p| p.desl), Some(1));
    }

    #[test]
    fn campos_de_autor_e_de_pagina() {
        let mut s = sessao(&["x"]);
        s.meta_definir("T", "Ana");
        s.contexto_definir(&ContextoExterno { hoje: "06/10/2026".into(), arquivo: "a.fdoz".into(), ..ContextoExterno::default() });
        ir(&mut s, 0, 1);
        s.objeto_inserir(Marca::Campo { campo: "autor".into() }).unwrap();
        s.objeto_inserir(Marca::Campo { campo: "pagina".into() }).unwrap();
        s.objeto_inserir(Marca::Campo { campo: "paginas".into() }).unwrap();
        s.objeto_inserir(Marca::Campo { campo: "data".into() }).unwrap();
        let t = &s.documento().blocos[0].conteudo.textos()[0].clone();
        let r: Vec<&str> = t.iter().filter(|x| x.marca.is_some()).map(|x| x.rotulo.as_str()).collect();
        assert_eq!(r, ["Ana", "1", "1", "06/10/2026"]);
    }
}
