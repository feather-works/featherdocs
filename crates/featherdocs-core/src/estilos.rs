//! Os estilos de parágrafo com nome e a numeração automática dos títulos.
//!
//! Um estilo tem uma base (Normal ou Título 1–6), que decide o tipo do bloco e o nível no sumário,
//! e o padrão da letra (fonte, corpo, cor, negrito…), que a composição lê na hora (`Entorno`): a
//! formatação feita à mão no texto continua valendo por cima, como no Word. O que é do parágrafo
//! (alinhamento, espaços, entrelinha) é carimbado no bloco ao aplicar; ao modificar o estilo, só
//! é recarimbado nos blocos que ainda estavam como o estilo deixou.

use crate::cursor::Cursor;
use crate::digitacao::tem_texto_corrido;
use crate::edicao::converter;
use crate::modelo::{base_do_conteudo, Atrib, Bloco, Estilo, Familia, Numeracao, CODIGO, SERIFA};
use crate::sessao::{Mudanca, Sessao, Splice};

/// O tipo de bloco da base do estilo (`paragrafo`, `titulo1`…).
fn tipo_da_base(base: u8) -> String {
    if base == 0 {
        "paragrafo".to_string()
    } else {
        format!("titulo{}", base.min(6))
    }
}

/// O que o estilo carimba no parágrafo.
fn carimbar(a: &mut Atrib, e: &Estilo) {
    a.estilo = e.id;
    a.alinhamento = e.alinhamento;
    a.antes = e.antes;
    a.depois = e.depois;
    a.entrelinha = e.entrelinha;
}

/// O parágrafo ainda está como o estilo `e` o deixou (ninguém mexeu à mão no que é do parágrafo).
fn como_o_estilo_deixou(a: &Atrib, e: &Estilo) -> bool {
    a.alinhamento == e.alinhamento && a.antes == e.antes && a.depois == e.depois && a.entrelinha == e.entrelinha
}

/// Converte o bloco para a base do estilo e carimba; `false` se o bloco não aceita estilo (tabela, imagem…).
fn aplicar_no_bloco(b: &mut Bloco, e: &Estilo) -> bool {
    if !tem_texto_corrido(&b.conteudo) {
        return false;
    }
    if base_do_conteudo(&b.conteudo) != Some(e.base) {
        let Ok(novo) = converter(&b.conteudo, &tipo_da_base(e.base)) else { return false };
        b.conteudo = novo;
        b.atrib.limpar_lista();
    }
    carimbar(&mut b.atrib, e);
    true
}

fn validar(e: &Estilo) -> Result<(), String> {
    if e.nome.trim().is_empty() {
        return Err("O estilo precisa de um nome.".to_string());
    }
    if e.base > 6 {
        return Err("A base do estilo vai de Normal a Título 6.".to_string());
    }
    if e.tamanho < 0.0 || e.tamanho > 400.0 || !e.tamanho.is_finite() {
        return Err("O tamanho da letra vai de 1 a 400 pt.".to_string());
    }
    if [e.antes, e.depois].into_iter().flatten().any(|v| !(0.0..=1000.0).contains(&v)) {
        return Err("O espaço antes e depois vai de 0 a 1000 pt.".to_string());
    }
    if e.entrelinha != 0.0 && !(0.5..=5.0).contains(&e.entrelinha) {
        return Err("A entrelinha vai de 0,5 a 5.".to_string());
    }
    Ok(())
}

impl Sessao {
    /// Os blocos `[i0, i1]` trocados por `dentro`, com o que os estilos eram antes (para o desfazer).
    fn fechar_estilos(&mut self, i0: usize, dentro: Vec<Bloco>, antes: crate::modelo::Estilos) -> Mudanca {
        let cursor: Cursor = self.cursor;
        let quantos = dentro.len();
        self.transacao_de_estilos(Splice { pos: i0, quantos, dentro }, cursor, antes)
    }

    /// Aplica o estilo `id` aos parágrafos tocados pela seleção.
    pub fn estilo_aplicar(&mut self, id: u32) -> Result<Mudanca, String> {
        let e = self.doc.estilos.pegar(id).cloned().ok_or("Esse estilo não existe mais.")?;
        let (mut t, _, _) = self.trabalho_da_selecao();
        let mut mudou = false;
        for b in &mut t.blocos {
            let antes = b.clone();
            mudou |= aplicar_no_bloco(b, &e) && *b != antes;
        }
        if !mudou {
            return Ok(self.sem_mudanca());
        }
        let cursor = self.cursor;
        Ok(self.fechar_trabalho(t, cursor, None))
    }

    /// Cria um estilo (o `id` de `e` é ignorado) e já o aplica aos parágrafos da seleção.
    pub fn estilo_criar(&mut self, mut e: Estilo) -> Result<Mudanca, String> {
        validar(&e)?;
        e.nome = e.nome.trim().to_string();
        if self.doc.estilos.lista.iter().any(|x| x.nome.to_lowercase() == e.nome.to_lowercase()) {
            return Err(format!("Já existe um estilo chamado \"{}\".", e.nome));
        }
        e.id = self.doc.estilos.lista.iter().map(|x| x.id).max().unwrap_or(0) + 1;
        let antes = self.doc.estilos.clone();
        self.doc.estilos.lista.push(e.clone());
        let (mut t, _, _) = self.trabalho_da_selecao();
        for b in &mut t.blocos {
            aplicar_no_bloco(b, &e);
        }
        Ok(self.fechar_estilos(t.pos, t.blocos, antes))
    }

    /// Muda o estilo `id`: todo parágrafo que o usa muda junto (o tipo, se a base mudou; o que é do
    /// parágrafo, se ninguém tinha mexido nele à mão; a letra, sempre, pela composição).
    pub fn estilo_modificar(&mut self, id: u32, mut novo: Estilo) -> Result<Mudanca, String> {
        validar(&novo)?;
        novo.nome = novo.nome.trim().to_string();
        let k = self.doc.estilos.lista.iter().position(|x| x.id == id).ok_or("Esse estilo não existe mais.")?;
        if self.doc.estilos.lista.iter().any(|x| x.id != id && x.nome.to_lowercase() == novo.nome.to_lowercase()) {
            return Err(format!("Já existe um estilo chamado \"{}\".", novo.nome));
        }
        let velho = self.doc.estilos.lista[k].clone();
        // O que o diálogo não mostra (de onde o estilo veio do Word, a herança, a fonte com nome) fica;
        // trocar a classe da fonte (serifa, código) tira o nome.
        let classe = SERIFA | CODIGO;
        let familia = if novo.bits & classe == velho.bits & classe { velho.familia } else { Familia::default() };
        let novo = Estilo { id, familia, codigo: velho.codigo.clone(), caractere: velho.caractere, herda: velho.herda, proximo: velho.proximo, realce: velho.realce, posicao: velho.posicao, fundo: velho.fundo, recuo_esq: velho.recuo_esq, recuo_dir: velho.recuo_dir, primeira: velho.primeira, ..novo };
        let antes = self.doc.estilos.clone();
        self.doc.estilos.lista[k] = novo.clone();
        // Só os blocos que mudam de fato entram no splice (da primeira à última posição tocada).
        let mut trocados: Vec<(usize, Bloco)> = Vec::new();
        for (i, b) in self.doc.blocos.iter().enumerate().filter(|(_, b)| b.atrib.estilo == id) {
            let mut n = b.clone();
            if base_do_conteudo(&n.conteudo) == Some(velho.base) && novo.base != velho.base {
                if let Ok(c) = converter(&n.conteudo, &tipo_da_base(novo.base)) {
                    n.conteudo = c;
                }
            }
            if como_o_estilo_deixou(&n.atrib, &velho) {
                carimbar(&mut n.atrib, &novo);
            }
            if n != *b {
                trocados.push((i, n));
            }
        }
        let (i0, dentro) = match (trocados.first(), trocados.last()) {
            (Some(&(i0, _)), Some(&(i1, _))) => {
                let mut dentro = self.doc.blocos[i0..=i1].to_vec();
                for (i, n) in trocados {
                    dentro[i - i0] = n;
                }
                (i0, dentro)
            }
            _ => (0, Vec::new()),
        };
        Ok(self.fechar_estilos(i0, dentro, antes))
    }

    /// Apaga o estilo `id`; os parágrafos que o usavam ficam com a base dele, sem o nome.
    pub fn estilo_excluir(&mut self, id: u32) -> Result<Mudanca, String> {
        let k = self.doc.estilos.lista.iter().position(|x| x.id == id).ok_or("Esse estilo não existe mais.")?;
        let antes = self.doc.estilos.clone();
        self.doc.estilos.lista.remove(k);
        let usam: Vec<usize> = self.doc.blocos.iter().enumerate().filter(|(_, b)| b.atrib.estilo == id).map(|(i, _)| i).collect();
        let (i0, dentro) = match (usam.first(), usam.last()) {
            (Some(&i0), Some(&i1)) => {
                let mut dentro = self.doc.blocos[i0..=i1].to_vec();
                for b in dentro.iter_mut().filter(|b| b.atrib.estilo == id) {
                    b.atrib.estilo = 0;
                }
                (i0, dentro)
            }
            _ => (0, Vec::new()),
        };
        Ok(self.fechar_estilos(i0, dentro, antes))
    }

    /// Liga a numeração dos títulos: `nenhuma`, `decimal` (1, 1.1, 1.1.1) ou `capitulo` (Capítulo 1, 1.1…).
    pub fn numeracao_definir(&mut self, nome: &str) -> Result<Mudanca, String> {
        let n = match nome {
            "nenhuma" => Numeracao::Nenhuma,
            "decimal" => Numeracao::Decimal,
            "capitulo" => Numeracao::Capitulo,
            outro => return Err(format!("Não conheço a numeração \"{outro}\".")),
        };
        if self.doc.estilos.numeracao == n {
            return Ok(self.sem_mudanca());
        }
        let antes = self.doc.estilos.clone();
        self.doc.estilos.numeracao = n;
        Ok(self.fechar_estilos(0, Vec::new(), antes))
    }

    /// Tira (ou devolve) os títulos tocados pela seleção da numeração automática.
    pub fn titulo_sem_numero(&mut self) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        let titulos = || t.blocos.iter().filter(|b| base_do_conteudo(&b.conteudo).is_some_and(|n| n > 0));
        if titulos().next().is_none() {
            return self.sem_mudanca();
        }
        let liga = !titulos().all(|b| b.atrib.sem_numero);
        for b in t.blocos.iter_mut().filter(|b| base_do_conteudo(&b.conteudo).is_some_and(|n| n > 0)) {
            b.atrib.sem_numero = liga;
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }

    /// O estilo com nome do parágrafo no cursor (0 = nenhum).
    pub fn estilo_com_nome_no_cursor(&self) -> u32 {
        self.idx(self.cursor.foco.bloco).and_then(|i| self.doc.estilos.do_bloco(&self.doc.blocos[i])).map_or(0, |e| e.id)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::{Alinhamento, Conteudo, Documento, NEGRITO, SERIFA};

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn capa(base: u8) -> Estilo {
        Estilo { id: 0, nome: "Capa".into(), base, bits: NEGRITO | SERIFA, tamanho: 28.0, cor: 0x3366_99ff, alinhamento: Alinhamento::Centro, antes: Some(0.0), depois: Some(30.0), entrelinha: 1.0, ..Estilo::default() }
    }

    #[test]
    fn heranca_e_padrao_do_documento_se_somam_e_o_motor_desenha_com_eles() {
        use crate::modelo::{Familia, ITALICO};
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: analisar("sem estilo") });
        d.acrescentar(Conteudo::Paragrafo { texto: analisar("filho") });
        d.estilos.padrao = Estilo { familia: Familia::de("Georgia"), bits: SERIFA, tamanho: 12.0, ..Estilo::default() };
        let pai = Estilo { id: 1, nome: "Pai".into(), codigo: "Pai".into(), bits: NEGRITO, familia: Familia::de("Calibri"), ..Estilo::default() };
        let filho = Estilo { id: 2, nome: "Filho".into(), codigo: "Filho".into(), herda: 1, bits: ITALICO, cor: 0x00b0_50ff, ..Estilo::default() };
        d.estilos.lista = vec![pai, filho];
        d.blocos[1].atrib.estilo = 2;
        let r = d.estilos.resolvido(2).unwrap();
        assert_eq!((r.bits, r.familia.nome(), r.tamanho, r.cor), (NEGRITO | ITALICO, "Calibri", 12.0, 0x00b0_50ff), "a fonte do pai troca a classe do padrão");
        assert_eq!(d.estilos.resolvido_do_bloco(&d.blocos[0]).map(|e| e.tamanho), Some(12.0), "o parágrafo sem estilo usa o padrão");
        let s = Sessao::com_documento(d);
        let (sem, com) = (&s.layouts[0].linhas[0].pedacos[0], &s.layouts[1].linhas[0].pedacos[0]);
        assert_eq!((sem.tamanho, sem.estilo & SERIFA), (12.0, SERIFA));
        assert_eq!((com.estilo & (NEGRITO | ITALICO | SERIFA), com.cor), (NEGRITO | ITALICO, 0x00b0_50ff));
    }

    #[test]
    fn criar_aplica_no_paragrafo_do_cursor_e_muda_a_letra() {
        let mut s = doc(&["Relatório", "texto"]);
        let antes = s.layouts[0].linhas[0].pedacos[0].tamanho;
        s.estilo_criar(capa(1)).unwrap();
        let b = &s.documento().blocos[0];
        assert!(matches!(b.conteudo, Conteudo::Titulo { nivel: 1, .. }), "a base Título 1 vira título");
        assert_eq!((b.atrib.estilo, b.atrib.alinhamento), (1, Alinhamento::Centro));
        let p = &s.layouts[0].linhas[0].pedacos[0];
        assert_eq!((p.tamanho, p.cor, p.estilo & SERIFA != 0), (28.0, 0x3366_99ff, true));
        assert_ne!(antes, p.tamanho);
        assert_eq!(s.estrutura().len(), 1, "título pela base: entra no sumário e na estrutura");
        assert_eq!(s.estilo_com_nome_no_cursor(), 1);
    }

    #[test]
    fn nome_vazio_ou_repetido_e_recusado() {
        let mut s = doc(&["a"]);
        assert!(s.estilo_criar(Estilo { nome: " ".into(), ..capa(0) }).is_err());
        s.estilo_criar(capa(0)).unwrap();
        assert!(s.estilo_criar(Estilo { nome: "capa".into(), ..capa(0) }).unwrap_err().contains("Já existe"));
    }

    #[test]
    fn modificar_muda_todos_mas_guarda_o_que_foi_feito_a_mao() {
        let mut s = doc(&["um", "dois"]);
        s.selecionar_tudo();
        s.estilo_criar(capa(0)).unwrap();
        // À mão: o segundo parágrafo foi alinhado à direita.
        let mut d = s.documento().clone();
        d.blocos[1].atrib.alinhamento = Alinhamento::Direita;
        let mut s = Sessao::com_documento(d);
        let m = s.estilo_modificar(1, Estilo { tamanho: 14.0, alinhamento: Alinhamento::Justificado, base: 2, ..capa(0) }).unwrap();
        let bs = &s.documento().blocos;
        assert_eq!(bs[0].atrib.alinhamento, Alinhamento::Justificado);
        assert_eq!(bs[1].atrib.alinhamento, Alinhamento::Direita, "o alinhamento feito à mão fica");
        assert!(bs.iter().all(|b| matches!(b.conteudo, Conteudo::Titulo { nivel: 2, .. })), "a base nova vale para todos");
        assert_eq!(s.layouts[1].linhas[0].pedacos[0].tamanho, 14.0);
        assert_eq!(m.afetados.len(), 2);
        // Desfazer volta o estilo e os blocos.
        s.desfazer();
        assert_eq!(s.documento().estilos.lista[0].tamanho, 28.0);
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Paragrafo { .. }));
        assert_eq!(s.layouts[0].linhas[0].pedacos[0].tamanho, 28.0);
    }

    #[test]
    fn excluir_devolve_os_paragrafos_a_base() {
        let mut s = doc(&["um"]);
        s.estilo_criar(capa(1)).unwrap();
        s.estilo_excluir(1).unwrap();
        let b = &s.documento().blocos[0];
        assert_eq!(b.atrib.estilo, 0);
        assert!(matches!(b.conteudo, Conteudo::Titulo { nivel: 1, .. }));
        assert_eq!(s.layouts[0].linhas[0].pedacos[0].tamanho, 24.0, "volta à letra do Título 1");
        s.desfazer();
        assert_eq!(s.documento().estilos.lista.len(), 1);
        assert_eq!(s.layouts[0].linhas[0].pedacos[0].tamanho, 28.0);
    }

    #[test]
    fn linha_do_sumario_leva_ao_titulo() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::sumario(3));
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Introdução") });
        d.acrescentar(Conteudo::Titulo { nivel: 2, texto: analisar("Objetivo") });
        let s = Sessao::com_documento(d);
        let f = &s.paginas[0].fatias[0];
        let l = &s.layouts[0].linhas;
        // A terceira linha (depois do título "Sumário" e de "Introdução") é a de "Objetivo".
        let y = s.documento().pagina.margem_sup + f.y + l[0].altura + l[1].altura + l[2].altura / 2.0;
        assert_eq!(s.alvo_do_sumario(0, y), Some(s.documento().blocos[2].id));
        assert_eq!(s.alvo_do_sumario(0, s.documento().pagina.margem_sup + f.y + 1.0), None);
    }

    #[test]
    fn numeracao_desenha_o_numero_e_entra_no_sumario() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::sumario(3));
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Introdução") });
        d.acrescentar(Conteudo::Titulo { nivel: 2, texto: analisar("Objetivo") });
        let mut s = Sessao::com_documento(d);
        let x_antes = s.layouts[1].linhas[0].x0;
        let m = s.numeracao_definir("decimal").unwrap();
        assert_eq!(m.afetados.len(), 2, "os títulos renumerados repintam");
        let n = s.layouts[2].numero.clone().unwrap();
        assert_eq!(n.texto, "1.1");
        assert!(s.layouts[1].linhas[0].x0 > x_antes, "o texto anda para dar lugar ao número");
        assert_eq!(s.estrutura()[1].texto, "1.1  Objetivo");
        s.numeracao_definir("capitulo").unwrap();
        assert_eq!(s.layouts[1].numero.as_ref().unwrap().texto, "Capítulo 1");
        s.desfazer();
        s.desfazer();
        assert!(s.layouts[1].numero.is_none());
    }

    #[test]
    fn titulo_fora_da_numeracao_e_os_seguintes_renumeram() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("A") });
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("B") });
        let mut s = Sessao::com_documento(d);
        s.numeracao_definir("decimal").unwrap();
        s.titulo_sem_numero();
        assert!(s.layouts[0].numero.is_none());
        assert_eq!(s.layouts[1].numero.as_ref().unwrap().texto, "1");
    }

    #[test]
    fn estilo_de_fabrica_tira_o_estilo_com_nome() {
        let mut s = doc(&["a"]);
        s.estilo_criar(capa(0)).unwrap();
        s.paragrafo_estilo("paragrafo");
        assert_eq!(s.documento().blocos[0].atrib.estilo, 0);
    }

    #[test]
    fn o_filho_desliga_o_que_o_pai_liga() {
        use crate::modelo::{Estilos, ALINHAMENTO_DEFINIDO, ITALICO, MANTER_COM_PROXIMO, MaisLetra, MaisParagrafo, VERSALETE};
        let pai = Estilo { id: 1, nome: "Pai".into(), bits: NEGRITO | ITALICO, alinhamento: Alinhamento::Centro, mais_letra: MaisLetra { efeitos: VERSALETE, ..MaisLetra::default() }, mais_paragrafo: MaisParagrafo { fluxo: MANTER_COM_PROXIMO, ..MaisParagrafo::default() }, ..Estilo::default() };
        let desligados = u32::from(NEGRITO) | u32::from(VERSALETE) << 8 | u32::from(MANTER_COM_PROXIMO) << 16 | ALINHAMENTO_DEFINIDO;
        let filho = Estilo { id: 2, nome: "Filho".into(), herda: 1, desligados, ..Estilo::default() };
        let neto = Estilo { id: 3, nome: "Neto".into(), herda: 2, bits: NEGRITO, ..Estilo::default() };
        let l = Estilos { lista: vec![pai, filho, neto], padrao: Estilo { bits: NEGRITO, ..Estilo::default() }, ..Estilos::default() };
        let f = l.resolvido(2).unwrap();
        assert_eq!(f.bits, ITALICO, "o negrito do pai (e do padrão) desliga; o itálico fica");
        assert_eq!(f.alinhamento, Alinhamento::Esquerda, "o alinhamento à esquerda do filho desfaz o centro do pai");
        assert_eq!((f.mais_letra.efeitos, f.mais_paragrafo.fluxo), (0, 0));
        assert_eq!(l.resolvido(3).unwrap().bits, NEGRITO | ITALICO, "o neto liga de novo");
        assert_eq!(l.resolvido(1).unwrap().alinhamento, Alinhamento::Centro);
    }

    #[test]
    fn paragrafo_com_nivel_de_topico_entra_no_sumario_e_na_estrutura() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::sumario(3));
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Capítulo") });
        let id = d.acrescentar(Conteudo::Paragrafo { texto: analisar("Tópico do Word") });
        d.blocos[2].atrib.mais.nivel_topico = 2;
        d.acrescentar(Conteudo::Paragrafo { texto: analisar("texto comum") });
        let s = Sessao::com_documento(d);
        let e = s.estrutura();
        assert_eq!(e.iter().map(|t| (t.nivel, t.texto.as_str())).collect::<Vec<_>>(), [(1, "Capítulo"), (2, "Tópico do Word")]);
        assert!(s.layouts[0].linhas.iter().any(|l| l.alvo == Some(id)), "o sumário gerado aponta para o parágrafo");
        assert!(matches!(s.documento().blocos[2].conteudo, Conteudo::Paragrafo { .. }), "continua parágrafo");
    }
}
