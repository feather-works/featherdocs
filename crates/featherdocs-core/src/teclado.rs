//! O que cada tecla faz. A interface repassa o nome da tecla (`KeyboardEvent.key`) e
//! os modificadores; quem decide é o motor. Texto digitado não passa por aqui
//! (vai por `inserir_texto`, que aceita acento morto e IME); copiar, recortar e
//! colar também não (vêm dos eventos de área de transferência).

use crate::cursor::{Cursor, Movimento};
use crate::modelo::{CODIGO, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
use crate::sessao::{Mudanca, Sessao};

/// A resposta de uma tecla: se o motor a usou (a interface então cancela a ação padrão do navegador) e o que mudou.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Teclado {
    pub tratada: bool,
    pub mudanca: Mudanca,
}

fn movimento_da_tecla(tecla: &str, ctrl: bool) -> Option<Movimento> {
    Some(match (tecla, ctrl) {
        ("ArrowLeft", false) => Movimento::Esquerda,
        ("ArrowRight", false) => Movimento::Direita,
        ("ArrowLeft", true) => Movimento::PalavraEsquerda,
        ("ArrowRight", true) => Movimento::PalavraDireita,
        ("ArrowUp", _) => Movimento::Cima,
        ("ArrowDown", _) => Movimento::Baixo,
        ("Home", false) => Movimento::InicioDaLinha,
        ("End", false) => Movimento::FimDaLinha,
        ("Home", true) => Movimento::InicioDoDocumento,
        ("End", true) => Movimento::FimDoDocumento,
        ("PageUp", _) => Movimento::PaginaAcima,
        ("PageDown", _) => Movimento::PaginaAbaixo,
        _ => return None,
    })
}

impl Sessao {
    /// Uma tecla de comando. `tratada = false` deixa o navegador agir (copiar, colar, F5…).
    pub fn tecla(&mut self, tecla: &str, ctrl: bool, shift: bool, alt: bool) -> Teclado {
        if let Some(m) = movimento_da_tecla(tecla, ctrl) {
            self.mover(m, shift);
            return Teclado { tratada: true, mudanca: self.sem_mudanca() };
        }
        match self.tecla_de_edicao(tecla, ctrl, shift, alt) {
            Some(mudanca) => Teclado { tratada: true, mudanca },
            None => Teclado { tratada: false, mudanca: self.sem_mudanca() },
        }
    }

    fn tecla_de_edicao(&mut self, tecla: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Mudanca> {
        let minuscula = tecla.to_lowercase();
        Some(match (tecla, ctrl, alt) {
            ("Backspace", c, false) => self.apagar(true, c),
            ("Delete", c, false) => self.apagar(false, c),
            ("Enter", true, false) => self.inserir_bloco("quebra_de_pagina"),
            ("Enter", false, _) if shift || alt => self.inserir_texto(&crate::modelo::QUEBRA_DE_LINHA.to_string()),
            ("Enter", false, false) => self.enter(),
            ("Tab", false, false) => self.tab(shift),
            ("Escape", false, false) => {
                let f = self.cursor.foco;
                self.cursor = Cursor::em(f);
                self.sem_mudanca()
            }
            _ if ctrl && !alt => return self.atalho_de_controle(&minuscula, shift),
            _ if ctrl && alt => return self.atalho_de_estilo(tecla),
            _ => return None,
        })
    }

    /// Enter: numa tabela desce uma linha (na última, cria uma nova embaixo); fora dela, quebra o
    /// parágrafo. Shift+Enter ou Alt+Enter quebram a linha dentro da célula.
    fn enter(&mut self) -> Mudanca {
        if let Some((l, _)) = self.cursor.foco.celula {
            if let Some((i, (linhas, colunas))) = self.idx(self.cursor.foco.bloco).and_then(|i| Some((i, crate::alvo::dimensoes(&self.doc.blocos[i].conteudo)?))) {
                if l as usize + 1 >= linhas {
                    return self.acrescentar_linha(i, colunas);
                }
            }
            self.mover(Movimento::Baixo, false);
            return self.sem_mudanca();
        }
        self.quebrar_paragrafo()
    }

    /// Ctrl+letra: formatação, desfazer, selecionar tudo. Copiar, recortar e colar ficam com o navegador.
    fn atalho_de_controle(&mut self, tecla: &str, shift: bool) -> Option<Mudanca> {
        Some(match (tecla, shift) {
            ("a", _) => {
                self.selecionar_tudo();
                self.sem_mudanca()
            }
            ("b", _) => self.formatar(NEGRITO),
            ("i", _) => self.formatar(ITALICO),
            ("u", _) => self.formatar(SUBLINHADO),
            ("e", true) => self.formatar(CODIGO),
            ("e", false) => self.alinhar("centro").unwrap_or_else(|_| self.sem_mudanca()),
            ("l", false) => self.alinhar("esquerda").unwrap_or_else(|_| self.sem_mudanca()),
            ("r", false) => self.alinhar("direita").unwrap_or_else(|_| self.sem_mudanca()),
            ("j", false) => self.alinhar("justificado").unwrap_or_else(|_| self.sem_mudanca()),
            ("x", true) => self.formatar(TACHADO),
            ("z", false) => self.desfazer().unwrap_or_else(|| self.sem_mudanca()),
            ("z", true) | ("y", _) => self.refazer().unwrap_or_else(|| self.sem_mudanca()),
            ("=", false) => self.posicao_alternar(-1),
            ("=", true) | ("+", _) => self.posicao_alternar(1),
            (" ", _) => self.limpar_formatacao(),
            (">" | ".", true) => self.tamanho_passo(1),
            ("<" | ",", true) => self.tamanho_passo(-1),
            ("c", true) => {
                self.pincel_copiar();
                self.sem_mudanca()
            }
            ("7", true) => self.paragrafo_estilo("item_ordenado"),
            ("8", true) => self.paragrafo_estilo("item"),
            _ => return None,
        })
    }

    /// Ctrl+Alt+0..6: texto normal e títulos.
    fn atalho_de_estilo(&mut self, tecla: &str) -> Option<Mudanca> {
        let tipo = match tecla {
            "0" => "paragrafo".to_string(),
            n @ ("1" | "2" | "3" | "4" | "5" | "6") => format!("titulo{n}"),
            _ => return None,
        };
        Some(self.paragrafo_estilo(&tipo))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::{Conteudo, Documento};

    fn doc(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        }
        Sessao::com_documento(d)
    }

    #[test]
    fn teclas_de_movimento_e_edicao() {
        let mut s = doc(&["abc"]);
        assert!(s.tecla("ArrowRight", false, false, false).tratada);
        assert!(s.tecla("ArrowRight", false, true, false).tratada);
        assert_eq!((s.cursor.ancora.desl, s.cursor.foco.desl), (1, 2));
        let r = s.tecla("Backspace", false, false, false);
        assert!(r.tratada && !r.mudanca.afetados.is_empty());
        assert_eq!(crate::texto::plano(&crate::alvo::texto_alvo(&s.documento().blocos[0].conteudo, None).unwrap()), "ac");
    }

    #[test]
    fn ctrl_c_e_ctrl_v_ficam_com_o_navegador() {
        let mut s = doc(&["abc"]);
        assert!(!s.tecla("c", true, false, false).tratada);
        assert!(!s.tecla("v", true, false, false).tratada);
        assert!(!s.tecla("F5", false, false, false).tratada);
    }

    #[test]
    fn ctrl_z_desfaz_e_ctrl_a_seleciona_tudo() {
        let mut s = doc(&["abc"]);
        s.inserir_texto("x");
        s.tecla("z", true, false, false);
        assert_eq!(crate::texto::plano(&crate::alvo::texto_alvo(&s.documento().blocos[0].conteudo, None).unwrap()), "abc");
        s.tecla("a", true, false, false);
        assert!(!s.cursor.vazio());
    }

    #[test]
    fn enter_na_ultima_linha_da_tabela_cria_outra_e_shift_quebra_a_linha() {
        let mut s = doc(&[""]);
        s.inserir_tabela(2, 2).unwrap();
        let tamanho = |s: &Sessao| match &s.documento().blocos.iter().find(|b| b.id == s.cursor.foco.bloco).unwrap().conteudo {
            Conteudo::Tabela { linhas, .. } => linhas.len(),
            _ => 0,
        };
        s.tecla("Enter", false, false, false);
        assert_eq!((tamanho(&s), s.cursor.foco.celula), (2, Some((1, 0))), "da primeira linha só desce");
        s.tecla("Tab", false, false, false);
        s.tecla("Enter", false, false, false);
        assert_eq!((tamanho(&s), s.cursor.foco.celula), (3, Some((2, 0))), "na última cria a linha e vai à primeira célula");
        s.inserir_texto("a");
        s.tecla("Enter", false, true, false);
        s.tecla("Enter", false, false, true);
        let Conteudo::Tabela { linhas, .. } = &s.documento().blocos.iter().find(|b| b.id == s.cursor.foco.bloco).unwrap().conteudo else { panic!() };
        let q = crate::modelo::QUEBRA_DE_LINHA;
        assert_eq!(crate::texto::plano(&linhas[2][0]), format!("a{q}{q}"));
        assert_eq!(tamanho(&s), 3);
    }

    #[test]
    fn ctrl_alt_numero_vira_titulo() {
        let mut s = doc(&["abc"]);
        s.tecla("2", true, false, true);
        assert!(matches!(s.documento().blocos[0].conteudo, Conteudo::Titulo { nivel: 2, .. }));
    }
}
