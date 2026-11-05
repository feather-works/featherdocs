//! Correção ao digitar, como no Word: aspas curvas, travessão, reticências e maiúscula no começo
//! da frase. Cada correção é um passo de desfazer só dela: Ctrl+Z logo depois devolve o que foi
//! digitado.

use crate::alvo::{caracteres_alvo, com_texto_ref, eh_atomico, eh_texto_cru};
use crate::cursor::{Cursor, Pos};
use crate::modelo::Trecho;
use crate::sessao::{Mudanca, Sessao};
use crate::texto::{dividir, fatia, juntar};

/// Palavras que terminam em ponto sem acabar a frase ("Sr. Silva", "p. 4", "etc. e").
const ABREVIACOES: &[&str] = &["sr", "sra", "srta", "dr", "dra", "prof", "profa", "etc", "ex", "obs", "pp", "fl", "fls", "art", "cf", "vol", "ed", "av", "tel", "cap", "fig", "tab", "eq", "núm", "num", "pág", "pag"];

impl Sessao {
    /// Liga ou desliga a correção ao digitar.
    pub fn definir_autocorrecao(&mut self, ligada: bool) {
        self.autocorrecao = ligada;
    }

    /// Logo depois de digitar `ch` no cursor: troca o que deve ser trocado, num passo de desfazer próprio.
    pub(crate) fn autocorrigir(&mut self, ch: char) -> Option<Mudanca> {
        if !self.autocorrecao || !self.cursor.vazio() || self.rastreando() {
            return None;
        }
        let foco = self.cursor.foco;
        let i = self.idx(foco.bloco)?;
        let c = &self.doc.blocos[i].conteudo;
        if eh_texto_cru(c) || eh_atomico(c) {
            return None;
        }
        let cs = caracteres_alvo(c, foco.celula);
        let fim = foco.desl;
        if fim == 0 || cs.get(fim - 1) != Some(&ch) {
            return None;
        }
        // A maiúscula só no começo de palavra nova: inserindo antes de um texto, não mexe.
        let depois = cs.get(fim).copied();
        let (n, novo) = troca(&cs[..fim], depois)?;
        let mut t = self.trabalho(i, i);
        com_texto_ref(&mut t.blocos[0].conteudo, foco.celula, |tr| trocar(tr, fim - n, fim, &novo));
        let desl = fim - n + novo.chars().count();
        Some(self.fechar_trabalho(t, Cursor::em(Pos { desl, ..foco }), None))
    }
}

/// O texto até o cursor (o último é o caractere digitado): quantos do fim trocar, e pelo quê.
fn troca(antes: &[char], depois: Option<char>) -> Option<(usize, String)> {
    let n = antes.len();
    let ch = antes[n - 1];
    let anterior = n.checked_sub(2).map(|k| antes[k]);
    match ch {
        '"' | '\'' => {
            let abre = anterior.map_or(true, |a| a.is_whitespace() || "([{—–/".contains(a));
            let aspa = match (ch, abre) {
                ('"', true) => '“',
                ('"', false) => '”',
                ('\'', true) => '‘',
                _ => '’',
            };
            Some((1, aspa.to_string()))
        }
        // "--" vira travessão; no começo do parágrafo, não (o "---" que vira linha horizontal).
        '-' if anterior == Some('-') && !antes.iter().all(|&c| c == '-') => Some((2, "—".into())),
        '.' if n >= 3 && antes[n - 3..] == ['.', '.', '.'] => Some((3, "…".into())),
        c if c.is_lowercase() && depois.map_or(true, char::is_whitespace) && comeca_frase(&antes[..n - 1]) => Some((1, c.to_uppercase().collect())),
        _ => None,
    }
}

/// O texto `antes` termina onde começa uma frase: vazio, ou fim de frase seguido de espaço.
fn comeca_frase(antes: &[char]) -> bool {
    let sem_espaco = antes.iter().rposition(|c| !c.is_whitespace());
    let Some(k) = sem_espaco else { return true };
    if k + 1 == antes.len() || !matches!(antes[k], '.' | '!' | '?') {
        return false;
    }
    if antes[k] != '.' {
        return true;
    }
    // A palavra antes do ponto: abreviação, sigla com pontos ("e.g.") ou inicial ("J.") não fecham frase.
    let ini = antes[..k].iter().rposition(|c| c.is_whitespace()).map_or(0, |p| p + 1);
    let palavra: String = antes[ini..k].iter().collect::<String>().to_lowercase();
    palavra.chars().count() > 1 && !palavra.contains('.') && palavra.chars().all(char::is_alphabetic) && !ABREVIACOES.contains(&palavra.as_str())
}

/// Troca os caracteres `[a, b)` por `novo`, com a formatação (link, revisão) do primeiro deles.
fn trocar(t: &mut Vec<Trecho>, a: usize, b: usize, novo: &str) {
    let Some(modelo) = fatia(t, a, b).into_iter().next() else { return };
    let (antes, _) = dividir(t, a);
    let resto = fatia(t, b, usize::MAX);
    *t = juntar(juntar(antes, vec![Trecho { texto: novo.to_string(), ..modelo }]), resto);
}

#[cfg(test)]
mod testes {
    use super::*;

    fn digitar(s: &mut Sessao, texto: &str) {
        for ch in texto.chars() {
            s.inserir_texto(&ch.to_string());
        }
    }

    fn texto(s: &Sessao) -> String {
        caracteres_alvo(&s.documento().blocos[0].conteudo, None).into_iter().collect()
    }

    fn sessao() -> Sessao {
        let mut s = Sessao::default();
        s.definir_autocorrecao(true);
        s
    }

    #[test]
    fn aspas_travessao_reticencias_e_maiuscula() {
        let mut s = sessao();
        digitar(&mut s, "ela disse \"oi\" -- e foi... depois. voltou d'água. Sr. silva, p. 4.");
        assert_eq!(texto(&s), "Ela disse “oi” — e foi… depois. Voltou d’água. Sr. silva, p. 4.");
    }

    #[test]
    fn ctrl_z_logo_depois_desfaz_so_a_correcao() {
        let mut s = sessao();
        digitar(&mut s, "a");
        assert_eq!(texto(&s), "A");
        s.desfazer();
        assert_eq!(texto(&s), "a");
    }

    #[test]
    fn desligada_e_no_comeco_do_paragrafo_nao_troca() {
        let mut s = Sessao::default();
        digitar(&mut s, "ola \"x\"");
        assert_eq!(texto(&s), "ola \"x\"");
        let mut s = sessao();
        digitar(&mut s, "---");
        assert_eq!(texto(&s), "---", "o atalho da linha horizontal continua");
    }
}
