//! Copiar, recortar e colar. O texto puro vai e vem pela área de transferência do
//! sistema; o aplicativo guarda também uma cópia com a formatação, que volta
//! quando o que se cola é exatamente o que se copiou aqui dentro.

use crate::alvo::{com_texto_ref, definir_texto, eh_atomico, tamanho_alvo, texto_alvo};
use crate::cursor::{Cursor, Pos};
use crate::digitacao::tem_texto_corrido;
use crate::modelo::{Bloco, Conteudo};
use crate::sessao::{AreaInterna, Mudanca, Sessao};
use crate::texto::{dividir, fatia, juntar, plano, tamanho};

/// O que a seleção tem em cada bloco: o pedaço do texto, ou o bloco inteiro.
pub(crate) fn recortar_blocos(bs: &[Bloco], ini: Pos, fim: Pos) -> Vec<Conteudo> {
    let ultimo = bs.len() - 1;
    let mut saida = Vec::new();
    for (k, b) in bs.iter().enumerate() {
        let c = &b.conteudo;
        if !tem_texto_corrido(c) {
            if let (Conteudo::Tabela { .. }, 1, Some(t)) = (c, bs.len(), texto_alvo(c, ini.celula)) {
                saida.push(Conteudo::Paragrafo { texto: fatia(&t, ini.desl, fim.desl) });
            } else {
                saida.push(c.clone());
            }
            continue;
        }
        let a = if k == 0 { ini.desl } else { 0 };
        let z = if k == ultimo { fim.desl } else { tamanho_alvo(c, None) };
        let mut parte = c.clone();
        if let Some(t) = texto_alvo(c, None) {
            definir_texto(&mut parte, None, fatia(&t, a, z));
        }
        saida.push(parte);
    }
    saida
}

/// O texto puro de um bloco, para a área de transferência do sistema.
pub(crate) fn texto_puro_do_bloco(c: &Conteudo) -> String {
    match c {
        Conteudo::Tabela { linhas, .. } => linhas.iter().map(|l| l.iter().map(|x| crate::inline::texto_puro(x)).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n"),
        _ => texto_alvo(c, None).map(|t| crate::inline::texto_puro(&t)).unwrap_or_default(),
    }
}

impl Sessao {
    /// O texto selecionado, em texto puro (parágrafos separados por quebra de linha). Guarda a cópia com formatação.
    pub fn copiar(&mut self) -> String {
        if self.cursor.vazio() {
            return String::new();
        }
        let (t, ini, fim) = self.trabalho_da_selecao();
        let blocos = recortar_blocos(&t.blocos, ini, fim);
        let plano_total = blocos.iter().filter(|c| !eh_atomico(c)).map(texto_puro_do_bloco).collect::<Vec<_>>().join("\n");
        self.area = Some(AreaInterna { plano: plano_total.clone(), blocos });
        plano_total
    }

    /// O texto selecionado em texto puro, sem mexer na cópia guardada (para preencher caixas).
    pub fn texto_selecionado(&self) -> String {
        if self.cursor.vazio() {
            return String::new();
        }
        let (t, ini, fim) = self.trabalho_da_selecao();
        recortar_blocos(&t.blocos, ini, fim).iter().filter(|c| !eh_atomico(c)).map(texto_puro_do_bloco).collect::<Vec<_>>().join("\n")
    }

    /// Copia e apaga.
    pub fn recortar(&mut self) -> (String, Mudanca) {
        let texto = self.copiar();
        let m = self.apagar_selecao();
        (texto, m)
    }

    /// Cola. Se `texto` é o que foi copiado aqui dentro, volta com a formatação; senão é texto puro.
    pub fn colar(&mut self, texto: &str) -> Mudanca {
        let normal = texto.replace("\r\n", "\n").replace('\r', "\n");
        let blocos = self.area.as_ref().filter(|a| a.plano == normal).map(|a| a.blocos.clone());
        match blocos {
            Some(b) if !b.is_empty() => self.colar_blocos(b),
            _ => self.inserir_texto(&normal),
        }
    }

    /// Cola texto puro, sem a formatação guardada.
    pub fn colar_sem_formatacao(&mut self, texto: &str) -> Mudanca {
        self.inserir_texto(texto)
    }

    fn colar_blocos(&mut self, mut frag: Vec<Conteudo>) -> Mudanca {
        let vazia = self.cursor.vazio();
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        let pos = if vazia { Some(ini) } else { crate::digitacao::apagar_faixa(&mut t.blocos, ini, fim) };
        let Some(pos) = pos else { return self.inserir_texto(&frag.iter().filter_map(|c| texto_alvo(c, None)).map(|x| plano(&x)).collect::<Vec<_>>().join("\n")) };
        let Some(k) = t.blocos.iter().position(|b| b.id == pos.bloco) else { return self.sem_mudanca() };
        // Na célula ou no código, vários blocos não cabem: entram como texto, um por linha.
        if frag.len() > 1 && (pos.celula.is_some() || crate::alvo::eh_texto_cru(&t.blocos[k].conteudo)) {
            return self.inserir_texto(&frag.iter().filter_map(|c| texto_alvo(c, None)).map(|x| plano(&x)).collect::<Vec<_>>().join("\n"));
        }
        // Um pedaço de texto só: entra no meio do parágrafo, mantendo a formatação dele.
        if frag.len() == 1 && tem_texto_corrido(&frag[0]) && tem_texto_corrido(&t.blocos[k].conteudo) {
            let novo = texto_alvo(&frag[0], None).unwrap_or_default();
            let n = tamanho(&novo);
            com_texto_ref(&mut t.blocos[k].conteudo, pos.celula, |x| {
                let (e, d) = dividir(x, pos.desl);
                *x = juntar(juntar(e, novo), d);
            });
            return self.fechar_trabalho(t, Cursor::em(Pos { desl: pos.desl + n, ..pos }), None);
        }
        // Vários blocos: o primeiro continua o parágrafo do cursor, o último ganha o resto dele.
        let direita = self.dividir_em(&mut t.blocos, pos);
        let r = t.blocos.iter().position(|b| b.id == direita.bloco).unwrap_or(k + 1);
        let primeiro = frag.remove(0);
        let mut posicao = k + 1;
        if tem_texto_corrido(&primeiro) && tem_texto_corrido(&t.blocos[k].conteudo) {
            let x = texto_alvo(&primeiro, None).unwrap_or_default();
            com_texto_ref(&mut t.blocos[k].conteudo, None, |y| *y = juntar(std::mem::take(y), x));
        } else {
            let b = self.doc.novo_bloco(primeiro);
            t.blocos.insert(posicao, b);
            posicao += 1;
        }
        let ultimo = frag.pop();
        for c in frag {
            let b = self.doc.novo_bloco(c);
            t.blocos.insert(posicao, b);
            posicao += 1;
        }
        let r = r + (posicao - (k + 1));
        let mut cursor = Pos { bloco: t.blocos[r.min(t.blocos.len() - 1)].id, celula: None, desl: 0 };
        if let Some(u) = ultimo {
            if tem_texto_corrido(&u) {
                let x = texto_alvo(&u, None).unwrap_or_default();
                let n = tamanho(&x);
                let resto = texto_alvo(&t.blocos[r].conteudo, None).unwrap_or_default();
                let mut novo = u;
                definir_texto(&mut novo, None, juntar(x, resto));
                t.blocos[r].conteudo = novo;
                cursor.desl = n;
            } else {
                let b = self.doc.novo_bloco(u);
                t.blocos.insert(r, b);
            }
        }
        self.fechar_trabalho(t, Cursor::em(cursor), None)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cursor::Movimento;
    use crate::inline::analisar;
    use crate::modelo::{Documento, NEGRITO};

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    fn textos(s: &Sessao) -> Vec<String> {
        s.documento().blocos.iter().map(|b| texto_alvo(&b.conteudo, None).map(|t| plano(&t)).unwrap_or_default()).collect()
    }

    #[test]
    fn copiar_e_colar_dentro_do_app_mantem_a_formatacao() {
        let mut s = doc(&["um **dois** tres"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Direita, false);
        for _ in 0..4 {
            s.mover(Movimento::Direita, true);
        }
        let copiado = s.copiar();
        assert_eq!(copiado, "dois");
        s.mover(Movimento::FimDaLinha, false);
        s.colar(&copiado);
        let Conteudo::Paragrafo { texto } = &s.documento().blocos[0].conteudo else { panic!() };
        assert_eq!(plano(texto), "um dois trestdois".replace("trestdois", "tresdois"));
        assert!(texto.iter().filter(|t| t.estilo == NEGRITO).count() == 2, "{texto:?}");
    }

    #[test]
    fn colar_paragrafos_no_codigo_fica_codigo_e_na_ordem() {
        let mut d = Documento::vazio();
        for t in ["A", "B"] {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        d.acrescentar(Conteudo::Codigo { linguagem: String::new(), codigo: "xyz".into() });
        let mut s = Sessao::com_documento(d);
        s.mover(Movimento::Baixo, true);
        s.mover(Movimento::FimDaLinha, true);
        let copiado = s.copiar();
        s.mover(Movimento::Baixo, false);
        s.mover(Movimento::Baixo, false);
        s.mover(Movimento::InicioDaLinha, false);
        s.mover(Movimento::Direita, false);
        s.colar(&copiado);
        let b = s.documento().blocos.last().unwrap();
        assert!(matches!(&b.conteudo, Conteudo::Codigo { codigo, .. } if codigo == "xA\nByz"), "{:?}", b.conteudo);
    }

    #[test]
    fn colar_texto_vindo_de_fora_e_puro() {
        let mut s = doc(&["ab"]);
        s.mover(Movimento::Direita, false);
        s.colar("X\nY");
        assert_eq!(textos(&s), vec!["aX", "Yb"]);
    }

    #[test]
    fn recortar_varios_paragrafos_e_colar_em_outro_lugar() {
        let mut s = doc(&["aaa", "bbb", "ccc", "ddd"]);
        s.mover(Movimento::Direita, false);
        s.mover(Movimento::Baixo, true);
        s.mover(Movimento::Baixo, true);
        let (texto, _) = s.recortar();
        assert_eq!(texto, "aa\nbbb\nc");
        assert_eq!(textos(&s), vec!["acc", "ddd"]);
        s.mover(Movimento::FimDoDocumento, false);
        s.colar(&texto);
        assert_eq!(textos(&s), vec!["acc", "dddaa", "bbb", "c"]);
    }
}
