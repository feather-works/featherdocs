//! Escrever e apagar: digitar, colar texto, Enter, Backspace, Delete, Tab. Cada
//! comando é **uma** edição (um passo de desfazer, ou parte de uma digitação
//! contínua) sobre uma cópia de trabalho dos blocos tocados.

use crate::alvo::{caracteres_alvo, com_texto_ref, definir_texto, eh_atomico, eh_texto_cru, tamanho_alvo, texto_alvo};
use crate::cursor::{Cursor, Pos};
use crate::edicao::converter;
use crate::modelo::{Bloco, Conteudo};
use crate::sessao::{Mudanca, Sessao, Splice};
use crate::texto::{apagar, fatia, inserir, marca_em, juntar, palavra_anterior, proxima_palavra};

/// Os tipos de edição contínua (para agrupar no desfazer).
const DIGITAR: u8 = 0;
const APAGAR_ATRAS: u8 = 1;
const APAGAR_FRENTE: u8 = 2;

/// Os blocos `[pos, pos + quantos)` copiados para editar.
pub(crate) struct Trabalho {
    pub pos: usize,
    pub quantos: usize,
    pub blocos: Vec<Bloco>,
}

/// O bloco tem texto de parágrafo (não é atômico nem tabela).
pub(crate) fn tem_texto_corrido(c: &Conteudo) -> bool {
    !eh_atomico(c) && !matches!(c, Conteudo::Tabela { .. })
}

/// Remove o texto de `ini` até `fim` dos blocos `bs`, que cobrem exatamente a seleção.
/// Devolve onde o cursor fica; `None` se não sobrou bloco.
pub(crate) fn apagar_faixa(bs: &mut Vec<Bloco>, ini: Pos, fim: Pos) -> Option<Pos> {
    let n = bs.len();
    if n == 1 {
        if eh_atomico(&bs[0].conteudo) {
            bs.clear();
            return None;
        }
        com_texto_ref(&mut bs[0].conteudo, ini.celula, |t| apagar(t, ini.desl, fim.desl));
        return Some(ini);
    }
    let direita = texto_alvo(&bs[n - 1].conteudo, None).filter(|_| tem_texto_corrido(&bs[n - 1].conteudo)).map(|t| fatia(&t, fim.desl, usize::MAX));
    if tem_texto_corrido(&bs[0].conteudo) {
        let esquerda = texto_alvo(&bs[0].conteudo, None).map(|t| fatia(&t, 0, ini.desl)).unwrap_or_default();
        definir_texto(&mut bs[0].conteudo, None, juntar(esquerda, direita.unwrap_or_default()));
        bs.truncate(1);
        Some(ini)
    } else if let Some(d) = direita {
        definir_texto(&mut bs[n - 1].conteudo, None, d);
        let id = bs[n - 1].id;
        bs.drain(0..n - 1);
        Some(Pos { bloco: id, celula: None, desl: 0 })
    } else {
        bs.clear();
        None
    }
}

fn parte_do_trabalho(bs: &mut [Bloco], id: u64) -> Option<usize> {
    bs.iter().position(|b| b.id == id)
}

impl Sessao {
    /// Copia os blocos que a seleção toca; devolve também o começo e o fim dela, em ordem.
    pub(crate) fn trabalho_da_selecao(&self) -> (Trabalho, Pos, Pos) {
        let (ini, fim) = self.selecao_ordenada();
        let i0 = self.idx(ini.bloco).unwrap_or(0);
        let i1 = self.idx(fim.bloco).unwrap_or(i0).max(i0);
        (self.trabalho(i0, i1), ini, fim)
    }

    pub(crate) fn trabalho(&self, i0: usize, i1: usize) -> Trabalho {
        Trabalho { pos: i0, quantos: i1 - i0 + 1, blocos: self.doc.blocos[i0..=i1].to_vec() }
    }

    pub(crate) fn fechar_trabalho(&mut self, t: Trabalho, cursor: Cursor, continuo: Option<(u64, u8)>) -> Mudanca {
        self.transacao(Splice { pos: t.pos, quantos: t.quantos, dentro: t.blocos }, cursor, continuo)
    }

    /// Onde o cursor fica quando os blocos `[pos, pos + quantos)` somem: o começo do seguinte, ou o fim do anterior.
    fn vizinho_para_cursor(&self, pos: usize, quantos: usize) -> Option<Pos> {
        if pos + quantos < self.doc.blocos.len() {
            Some(self.inicio_do_bloco(pos + quantos))
        } else if pos > 0 {
            Some(self.fim_do_bloco(pos - 1))
        } else {
            None
        }
    }

    /// Um parágrafo vazio novo, para o documento nunca ficar sem bloco.
    fn paragrafo_vazio(&mut self) -> Bloco {
        self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() })
    }

    /// Fecha uma edição que pode ter apagado todos os blocos de trabalho.
    fn fechar_apagando(&mut self, mut t: Trabalho, pos: Option<Pos>, continuo: Option<(u64, u8)>) -> Mudanca {
        let cursor = match pos {
            Some(p) => p,
            None => match self.vizinho_para_cursor(t.pos, t.quantos) {
                Some(p) => p,
                None => {
                    let b = self.paragrafo_vazio();
                    let p = Pos { bloco: b.id, celula: None, desl: 0 };
                    t.blocos.push(b);
                    p
                }
            },
        };
        self.fechar_trabalho(t, Cursor::em(cursor), continuo)
    }

    /// Apaga o que está selecionado.
    pub fn apagar_selecao(&mut self) -> Mudanca {
        if self.cursor.vazio() {
            return self.sem_mudanca();
        }
        if let Some(m) = self.riscar_selecao_entre_blocos() {
            return m;
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        let pos = apagar_faixa(&mut t.blocos, ini, fim);
        self.fechar_apagando(t, pos, None)
    }

    /// Escreve `texto` no cursor (no lugar da seleção, se houver). Quebras de linha viram parágrafos.
    pub fn inserir_texto(&mut self, texto: &str) -> Mudanca {
        let texto = texto.replace("\r\n", "\n").replace('\r', "\n");
        if texto.is_empty() {
            return self.sem_mudanca();
        }
        if texto == " " && self.cursor.vazio() {
            if let Some(m) = self.autoformatar_espaco() {
                return m;
            }
        }
        // Sugerindo: a seleção entre parágrafos é riscada e o texto novo entra no fim dela.
        let _ = self.riscar_selecao_entre_blocos();
        let vazia = self.cursor.vazio();
        let pendente = self.cursor.pendente;
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        let mut pos = if vazia { Some(ini) } else { apagar_faixa(&mut t.blocos, ini, fim) };
        if pos.is_none() {
            let b = self.paragrafo_vazio();
            pos = Some(Pos { bloco: b.id, celula: None, desl: 0 });
            t.blocos.push(b);
        }
        let mut pos = pos.unwrap_or(ini);
        let comeca_palavra = vazia && !texto.starts_with(char::is_whitespace) && self.caractere_antes(&ini).is_some_and(char::is_whitespace);
        for (k, linha) in texto.split('\n').enumerate() {
            if k > 0 {
                pos = self.dividir_em(&mut t.blocos, pos);
            }
            pos = self.inserir_linha(&mut t.blocos, pos, linha, pendente);
        }
        let continuo = (vazia && !texto.contains('\n')).then_some((pos.bloco, DIGITAR));
        if comeca_palavra {
            self.digitando = None;
        }
        self.fechar_trabalho(t, Cursor::em(pos), continuo)
    }

    /// O caractere logo antes do cursor (no mesmo texto).
    fn caractere_antes(&self, p: &Pos) -> Option<char> {
        let i = self.idx(p.bloco)?;
        let c = &self.doc.blocos[i].conteudo;
        (p.desl > 0).then(|| caracteres_alvo(c, p.celula).get(p.desl - 1).copied()).flatten()
    }

    /// Escreve uma linha (sem quebras) em `pos`; devolve a posição depois dela.
    fn inserir_linha(&mut self, bs: &mut Vec<Bloco>, pos: Pos, linha: &str, pendente: Option<(u8, crate::modelo::Fmt)>) -> Pos {
        if linha.is_empty() {
            return pos;
        }
        let Some(k) = parte_do_trabalho(bs, pos.bloco) else { return pos };
        if eh_atomico(&bs[k].conteudo) {
            let novo = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
            let id = novo.id;
            bs.insert(k + 1, novo);
            return self.inserir_linha(bs, Pos { bloco: id, celula: None, desl: 0 }, linha, pendente);
        }
        let cru = eh_texto_cru(&bs[k].conteudo);
        let linha = if pos.celula.is_some() { linha.replace('\t', " ") } else { linha.to_string() };
        let n = linha.chars().count();
        com_texto_ref(&mut bs[k].conteudo, pos.celula, |t| {
            let (estilo, fmt) = if cru { (0, Default::default()) } else { pendente.unwrap_or_else(|| marca_em(t, pos.desl)) };
            inserir(t, pos.desl, &linha, estilo, fmt);
        });
        Pos { desl: pos.desl + n, ..pos }
    }

    /// Parte o bloco em `pos` (Enter): o que vem depois vai para um bloco novo. Devolve o começo do novo.
    pub(crate) fn dividir_em(&mut self, bs: &mut Vec<Bloco>, pos: Pos) -> Pos {
        let Some(k) = parte_do_trabalho(bs, pos.bloco) else { return pos };
        let c = &mut bs[k].conteudo;
        if eh_atomico(c) {
            let novo = self.paragrafo_vazio();
            let id = novo.id;
            bs.insert(k + 1, novo);
            return Pos { bloco: id, celula: None, desl: 0 };
        }
        if matches!(c, Conteudo::Tabela { .. }) {
            return pos;
        }
        if eh_texto_cru(c) {
            com_texto_ref(c, None, |t| inserir(t, pos.desl, "\n", 0, Default::default()));
            return Pos { desl: pos.desl + 1, ..pos };
        }
        let (esq, dir) = crate::texto::dividir(&texto_alvo(c, None).unwrap_or_default(), pos.desl);
        let novo_conteudo = match &*c {
            Conteudo::Item { ordenado, nivel, .. } => Conteudo::Item { ordenado: *ordenado, nivel: *nivel, texto: dir },
            Conteudo::Citacao { .. } => Conteudo::Citacao { texto: dir },
            _ => Conteudo::Paragrafo { texto: dir },
        };
        definir_texto(c, None, esq);
        let atrib = bs[k].atrib;
        let mut novo = self.doc.novo_bloco(novo_conteudo);
        novo.atrib = crate::modelo::Atrib { marcado: false, reiniciar: false, mais: crate::modelo::MaisParagrafo { inicio: None, ..atrib.mais }, ..atrib };
        let id = novo.id;
        bs.insert(k + 1, novo);
        Pos { bloco: id, celula: None, desl: 0 }
    }

    /// Enter.
    pub fn quebrar_paragrafo(&mut self) -> Mudanca {
        if self.cursor.vazio() {
            if let Some(m) = self.autoformatar_enter() {
                return m;
            }
            if let Some(m) = self.sair_do_item_vazio() {
                return m;
            }
        }
        self.inserir_texto("\n")
    }

    /// Enter num item de lista (ou citação) vazio: o item vira parágrafo, como em qualquer editor.
    fn sair_do_item_vazio(&mut self) -> Option<Mudanca> {
        let foco = self.cursor.foco;
        let i = self.idx(foco.bloco)?;
        let c = &self.doc.blocos[i].conteudo;
        if !matches!(c, Conteudo::Item { .. } | Conteudo::Citacao { .. }) || tamanho_alvo(c, None) > 0 {
            return None;
        }
        let novo = converter(c, "paragrafo").ok()?;
        let cursor = self.cursor;
        Some(self.transacao(Splice { pos: i, quantos: 1, dentro: vec![Bloco { atrib: self.doc.blocos[i].atrib, ..Bloco::novo(foco.bloco, novo) }] }, cursor, None))
    }

    /// Backspace (`para_tras`) ou Delete; `palavra` apaga a palavra inteira (Ctrl).
    pub fn apagar(&mut self, para_tras: bool, palavra: bool) -> Mudanca {
        if !self.cursor.vazio() {
            return self.apagar_selecao();
        }
        let foco = self.cursor.foco;
        let Some(i) = self.idx(foco.bloco) else { return self.sem_mudanca() };
        let c = &self.doc.blocos[i].conteudo;
        if eh_atomico(c) {
            return self.remover_blocos(i, i);
        }
        let total = tamanho_alvo(c, foco.celula);
        let texto = if palavra { caracteres_alvo(c, foco.celula) } else { Vec::new() };
        if para_tras && foco.desl > 0 {
            let de = if palavra { palavra_anterior(&texto, foco.desl) } else { foco.desl - 1 };
            return self.apagar_trecho(foco, de, foco.desl, APAGAR_ATRAS);
        }
        if !para_tras && foco.desl < total {
            let ate = if palavra { proxima_palavra(&texto, foco.desl) } else { foco.desl + 1 };
            return self.apagar_trecho(foco, foco.desl, ate, APAGAR_FRENTE);
        }
        if foco.celula.is_some() {
            return self.sem_mudanca();
        }
        if para_tras {
            self.apagar_no_inicio(i)
        } else {
            self.puxar_proximo(i)
        }
    }

    fn apagar_trecho(&mut self, foco: Pos, de: usize, ate: usize, tipo: u8) -> Mudanca {
        let (mut t, _, _) = self.trabalho_da_selecao();
        com_texto_ref(&mut t.blocos[0].conteudo, foco.celula, |x| apagar(x, de, ate));
        let cursor = Cursor::em(Pos { desl: de, ..foco });
        self.fechar_trabalho(t, cursor, Some((foco.bloco, tipo)))
    }

    /// Tira os blocos `i0..=i1` do documento.
    fn remover_blocos(&mut self, i0: usize, i1: usize) -> Mudanca {
        let t = Trabalho { pos: i0, quantos: i1 - i0 + 1, blocos: Vec::new() };
        self.fechar_apagando(t, None, None)
    }

    /// Backspace no começo de um bloco: vira parágrafo; se já é, junta com o anterior.
    fn apagar_no_inicio(&mut self, i: usize) -> Mudanca {
        let c = &self.doc.blocos[i].conteudo;
        let id = self.doc.blocos[i].id;
        if !matches!(c, Conteudo::Paragrafo { .. }) {
            if let Ok(novo) = converter(c, "paragrafo") {
                let cursor = self.cursor;
                return self.transacao(Splice { pos: i, quantos: 1, dentro: vec![Bloco { atrib: self.doc.blocos[i].atrib, ..Bloco::novo(id, novo) }] }, cursor, None);
            }
        }
        if i == 0 {
            return self.sem_mudanca();
        }
        let anterior = &self.doc.blocos[i - 1].conteudo;
        if eh_atomico(anterior) {
            return self.remover_blocos_sem_mexer_no_cursor(i - 1);
        }
        if matches!(anterior, Conteudo::Tabela { .. }) {
            let p = self.fim_do_bloco(i - 1);
            self.definir_foco(p, false, None);
            return self.sem_mudanca();
        }
        self.juntar_com_anterior(i)
    }

    /// Apaga um bloco atômico que estava antes do cursor, sem tirar o cursor do lugar.
    fn remover_blocos_sem_mexer_no_cursor(&mut self, i: usize) -> Mudanca {
        let cursor = self.cursor;
        self.transacao(Splice { pos: i, quantos: 1, dentro: Vec::new() }, cursor, None)
    }

    /// O texto do bloco `i` passa para o fim do bloco anterior.
    fn juntar_com_anterior(&mut self, i: usize) -> Mudanca {
        let mut t = self.trabalho(i - 1, i);
        let esquerda = tamanho_alvo(&t.blocos[0].conteudo, None);
        let direita = texto_alvo(&t.blocos[1].conteudo, None).unwrap_or_default();
        com_texto_ref(&mut t.blocos[0].conteudo, None, |x| *x = juntar(std::mem::take(x), direita));
        t.blocos.truncate(1);
        let cursor = Cursor::em(Pos { bloco: t.blocos[0].id, celula: None, desl: esquerda });
        self.fechar_trabalho(t, cursor, None)
    }

    /// Delete no fim de um bloco: o seguinte entra neste.
    fn puxar_proximo(&mut self, i: usize) -> Mudanca {
        if i + 1 >= self.doc.blocos.len() {
            return self.sem_mudanca();
        }
        let proximo = &self.doc.blocos[i + 1].conteudo;
        if eh_atomico(proximo) {
            return self.remover_blocos_sem_mexer_no_cursor(i + 1);
        }
        if matches!(proximo, Conteudo::Tabela { .. }) {
            return self.sem_mudanca();
        }
        let foco = self.cursor.foco;
        let mut t = self.trabalho(i, i + 1);
        let direita = texto_alvo(&t.blocos[1].conteudo, None).unwrap_or_default();
        com_texto_ref(&mut t.blocos[0].conteudo, None, |x| *x = juntar(std::mem::take(x), direita));
        t.blocos.truncate(1);
        self.fechar_trabalho(t, Cursor::em(foco), None)
    }

    /// Tab: recua o item de lista, ou anda entre as células da tabela, ou escreve um recuo.
    pub fn tab(&mut self, voltar: bool) -> Mudanca {
        let foco = self.cursor.foco;
        let Some(i) = self.idx(foco.bloco) else { return self.sem_mudanca() };
        match &self.doc.blocos[i].conteudo {
            Conteudo::Item { .. } => self.mudar_nivel(if voltar { -1 } else { 1 }),
            Conteudo::Tabela { .. } => self.celula_seguinte(i, voltar),
            _ if voltar => self.sem_mudanca(),
            _ => self.inserir_texto("    "),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cursor::Movimento;
    use crate::inline::analisar;
    use crate::modelo::Documento;
    use crate::texto::plano;

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn textos(s: &Sessao) -> Vec<String> {
        s.documento().blocos.iter().map(|b| texto_alvo(&b.conteudo, None).map(|t| plano(&t)).unwrap_or_else(|| "·".into())).collect()
    }

    #[test]
    fn digitar_poe_o_texto_no_cursor_e_o_cursor_anda() {
        let mut s = doc(&[""]);
        s.inserir_texto("Olá");
        s.inserir_texto(" mundo");
        assert_eq!(textos(&s), vec!["Olá mundo"]);
        assert_eq!(s.cursor.foco.desl, 9);
    }

    #[test]
    fn enter_parte_o_paragrafo_e_backspace_junta() {
        let mut s = doc(&["abcdef"]);
        for _ in 0..3 {
            s.mover(Movimento::Direita, false);
        }
        s.quebrar_paragrafo();
        assert_eq!(textos(&s), vec!["abc", "def"]);
        assert_eq!((s.idx(s.cursor.foco.bloco), s.cursor.foco.desl), (Some(1), 0));
        s.apagar(true, false);
        assert_eq!(textos(&s), vec!["abcdef"]);
        assert_eq!(s.cursor.foco.desl, 3);
    }

    #[test]
    fn delete_no_fim_puxa_o_proximo_paragrafo() {
        let mut s = doc(&["ab", "cd"]);
        s.mover(Movimento::FimDaLinha, false);
        s.apagar(false, false);
        assert_eq!(textos(&s), vec!["abcd"]);
        assert_eq!(s.cursor.foco.desl, 2);
    }

    #[test]
    fn apagar_selecao_entre_paragrafos_junta_as_pontas() {
        let mut s = doc(&["um dois", "tres", "quatro cinco"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Baixo, true);
        s.mover(Movimento::Baixo, true);
        s.mover(Movimento::Direita, true);
        s.mover(Movimento::Direita, true);
        s.mover(Movimento::Direita, true);
        s.apagar_selecao();
        assert_eq!(textos(&s), vec!["um  cinco"]);
        assert!(s.cursor.vazio());
        assert_eq!(s.cursor.foco.desl, 3);
        s.desfazer().unwrap();
        assert_eq!(textos(&s), vec!["um dois", "tres", "quatro cinco"]);
        assert_eq!(s.cursor.ancora.desl, 3, "desfazer devolve a seleção");
    }

    #[test]
    fn digitar_com_selecao_substitui_e_vira_um_passo() {
        let mut s = doc(&["abcdef"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, true);
        s.mover(Movimento::Direita, true);
        s.inserir_texto("XY");
        assert_eq!(textos(&s), vec!["aXYdef"]);
        s.desfazer().unwrap();
        assert_eq!(textos(&s), vec!["abcdef"]);
    }

    #[test]
    fn digitacao_agrupa_por_palavra_no_desfazer() {
        let mut s = doc(&[""]);
        for t in ["um", " ", "dois", " ", "tres"] {
            for c in t.chars() {
                s.inserir_texto(&c.to_string());
            }
        }
        assert_eq!(textos(&s), vec!["um dois tres"]);
        s.desfazer().unwrap();
        assert_eq!(textos(&s), vec!["um dois "]);
        s.desfazer().unwrap();
        assert_eq!(textos(&s), vec!["um "]);
        s.refazer().unwrap();
        assert_eq!(textos(&s), vec!["um dois "]);
    }

    #[test]
    fn colar_texto_com_quebras_cria_paragrafos() {
        let mut s = doc(&["ab"]);
        s.mover(Movimento::Direita, false);
        s.inserir_texto("1\n2\n3");
        assert_eq!(textos(&s), vec!["a1", "2", "3b"]);
        assert_eq!((s.idx(s.cursor.foco.bloco), s.cursor.foco.desl), (Some(2), 1));
    }

    #[test]
    fn backspace_no_inicio_de_item_vira_paragrafo_e_depois_junta() {
        let mut s = doc(&["a", "b"]);
        s.mover(Movimento::Baixo, false);
        s.paragrafo_estilo("item");
        assert!(matches!(s.documento().blocos[1].conteudo, Conteudo::Item { .. }));
        s.apagar(true, false);
        assert!(matches!(s.documento().blocos[1].conteudo, Conteudo::Paragrafo { .. }));
        s.apagar(true, false);
        assert_eq!(textos(&s), vec!["ab"]);
    }

    #[test]
    fn apagar_tudo_deixa_um_paragrafo_vazio() {
        let mut s = doc(&["a", "b"]);
        s.selecionar_tudo();
        s.apagar_selecao();
        assert_eq!(textos(&s), vec![""]);
    }
}
