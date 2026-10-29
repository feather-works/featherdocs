//! Operações sobre texto com formatação (`Vec<Trecho>`), sempre por posição em
//! **caracteres** (não bytes). É o que o cursor, a seleção e a digitação usam.

use crate::modelo::{Fmt, Marca, Trecho, MATEMATICA, OBJETO};

/// Quantos caracteres tem o texto.
pub fn tamanho(t: &[Trecho]) -> usize {
    t.iter().map(|x| x.texto.chars().count()).sum()
}

/// O texto puro.
pub fn plano(t: &[Trecho]) -> String {
    t.iter().map(|x| x.texto.as_str()).collect()
}

/// A posição em bytes do caractere `n` de `s` (o fim se `n` passa do tamanho).
fn byte_de(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

/// Tira trechos vazios e junta vizinhos com a mesma formatação.
pub fn normalizar(t: &mut Vec<Trecho>) {
    t.retain(|x| !x.texto.is_empty());
    let mut i = 1;
    while i < t.len() {
        let soltos = t[i - 1].marca.is_none() && t[i].marca.is_none() && t[i - 1].rev == t[i].rev;
        if soltos && t[i - 1].estilo == t[i].estilo && t[i - 1].link == t[i].link && t[i - 1].fmt == t[i].fmt {
            let depois = t.remove(i);
            t[i - 1].texto.push_str(&depois.texto);
        } else {
            i += 1;
        }
    }
}

/// O pedaço `[a, b)` do texto, com a formatação.
pub fn fatia(t: &[Trecho], a: usize, b: usize) -> Vec<Trecho> {
    let mut saida = Vec::new();
    let mut pos = 0;
    for x in t {
        let n = x.texto.chars().count();
        let (de, ate) = (a.max(pos), b.min(pos + n));
        if de < ate {
            let s = &x.texto[byte_de(&x.texto, de - pos)..byte_de(&x.texto, ate - pos)];
            saida.push(Trecho { texto: s.to_string(), ..x.clone() });
        }
        pos += n;
    }
    saida
}

/// Parte o texto em `desl`: o que vem antes e o que vem depois.
pub fn dividir(t: &[Trecho], desl: usize) -> (Vec<Trecho>, Vec<Trecho>) {
    (fatia(t, 0, desl), fatia(t, desl, usize::MAX))
}

/// `a` seguido de `b`.
pub fn juntar(mut a: Vec<Trecho>, b: Vec<Trecho>) -> Vec<Trecho> {
    a.extend(b);
    normalizar(&mut a);
    a
}

/// Tira o texto `[a, b)`.
pub fn apagar(t: &mut Vec<Trecho>, a: usize, b: usize) {
    if a >= b {
        return;
    }
    let (antes, _) = dividir(t, a);
    let depois = fatia(t, b, usize::MAX);
    *t = juntar(antes, depois);
}

/// A formatação que o texto digitado em `desl` herda: a do caractere de antes (ou a do primeiro, no
/// começo). A fórmula em linha não se herda (digitar depois dela não continua a fórmula).
pub fn marca_em(t: &[Trecho], desl: usize) -> (u8, Fmt) {
    let e = if desl > 0 { fatia(t, desl - 1, desl) } else { fatia(t, 0, 1) };
    e.first().map_or((0, Fmt::default()), |x| (x.estilo & !MATEMATICA, x.fmt))
}

/// O link no qual `desl` está **dentro** (o caractere de antes e o de depois são do mesmo link).
fn link_em(t: &[Trecho], desl: usize) -> Option<String> {
    let antes = fatia(t, desl.saturating_sub(1), desl).into_iter().next()?;
    let depois = fatia(t, desl, desl + 1).into_iter().next()?;
    (antes.link == depois.link).then_some(antes.link).flatten()
}

/// Insere `texto` em `desl` com a formatação `estilo` e `fmt`.
pub fn inserir(t: &mut Vec<Trecho>, desl: usize, texto: &str, estilo: u8, fmt: Fmt) {
    // O caractere de objeto só entra pelo `inserir_objeto` (com o objeto junto), nunca solto.
    let limpo;
    let texto = if texto.contains(OBJETO) {
        limpo = texto.replace(OBJETO, "");
        limpo.as_str()
    } else {
        texto
    };
    if texto.is_empty() {
        return;
    }
    let (antes, depois) = dividir(t, desl);
    let link = link_em(t, desl);
    let meio = vec![Trecho { texto: texto.to_string(), estilo, link, fmt, ..Trecho::default() }];
    *t = juntar(juntar(antes, meio), depois);
}

/// Prende um objeto (nota, citação, campo…) ao texto em `desl`, com a formatação de quem está ali.
pub fn inserir_objeto(t: &mut Vec<Trecho>, desl: usize, marca: Marca) {
    let (estilo, fmt) = marca_em(t, desl);
    let (antes, depois) = dividir(t, desl);
    let obj = Trecho { estilo, fmt, link: None, ..Trecho::objeto(marca) };
    *t = juntar(juntar(antes, vec![obj]), depois);
}

/// O objeto que ocupa o caractere `desl` (o que está logo depois do ponto), se houver.
pub fn objeto_em(t: &[Trecho], desl: usize) -> Option<&Marca> {
    let mut pos = 0;
    for x in t {
        let n = x.texto.chars().count();
        if desl < pos + n {
            return x.marca.as_deref();
        }
        pos += n;
    }
    None
}

/// Troca o objeto que ocupa o caractere `desl`; `false` se ali não há objeto.
pub fn trocar_objeto(t: &mut [Trecho], desl: usize, marca: Marca) -> bool {
    let mut pos = 0;
    for x in t.iter_mut() {
        let n = x.texto.chars().count();
        if desl < pos + n {
            if x.marca.is_none() {
                return false;
            }
            x.marca = Some(Box::new(marca));
            return true;
        }
        pos += n;
    }
    false
}

/// Liga ou desliga `bit` no texto `[a, b)`.
pub fn aplicar_estilo(t: &mut Vec<Trecho>, a: usize, b: usize, bit: u8, ligar: bool) {
    if a >= b {
        return;
    }
    let mut meio = fatia(t, a, b);
    for x in &mut meio {
        if ligar {
            x.estilo |= bit;
        } else {
            x.estilo &= !bit;
        }
    }
    let antes = fatia(t, 0, a);
    let depois = fatia(t, b, usize::MAX);
    *t = juntar(juntar(antes, meio), depois);
}

/// Muda cada trecho do texto `[a, b)` com `f`.
pub fn modificar(t: &mut Vec<Trecho>, a: usize, b: usize, f: impl Fn(&mut Trecho)) {
    if a >= b {
        return;
    }
    let mut meio = fatia(t, a, b);
    meio.iter_mut().for_each(f);
    let antes = fatia(t, 0, a);
    let depois = fatia(t, b, usize::MAX);
    *t = juntar(juntar(antes, meio), depois);
}

/// Todo o texto `[a, b)` tem `bit`? (Texto vazio: não.)
pub fn todo_com_estilo(t: &[Trecho], a: usize, b: usize, bit: u8) -> bool {
    let meio = fatia(t, a, b);
    !meio.is_empty() && meio.iter().all(|x| x.estilo & bit != 0)
}

fn eh_letra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A classe do caractere para achar palavra: letra, espaço ou pontuação.
fn classe(c: char) -> u8 {
    if eh_letra(c) {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

/// A palavra (ou a sequência de espaços/pontuação) que contém `desl`: `[a, b)`.
pub fn palavra_em(texto: &[char], desl: usize) -> (usize, usize) {
    if texto.is_empty() {
        return (0, 0);
    }
    let i = desl.min(texto.len() - 1);
    let k = classe(texto[i]);
    let a = (0..=i).rev().take_while(|&j| classe(texto[j]) == k).last().unwrap_or(i);
    let b = (i..texto.len()).take_while(|&j| classe(texto[j]) == k).last().map_or(i, |j| j + 1);
    (a, b)
}

/// O começo da próxima palavra (Ctrl+→).
pub fn proxima_palavra(texto: &[char], desl: usize) -> usize {
    let mut i = desl.min(texto.len());
    while i < texto.len() && eh_letra(texto[i]) {
        i += 1;
    }
    while i < texto.len() && !eh_letra(texto[i]) {
        i += 1;
    }
    i
}

/// O começo da palavra anterior (Ctrl+←).
pub fn palavra_anterior(texto: &[char], desl: usize) -> usize {
    let mut i = desl.min(texto.len());
    while i > 0 && !eh_letra(texto[i - 1]) {
        i -= 1;
    }
    while i > 0 && eh_letra(texto[i - 1]) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::{NEGRITO, ITALICO};

    fn t(s: &str) -> Vec<Trecho> {
        analisar(s)
    }

    #[test]
    fn inserir_e_apagar_em_posicoes_de_caractere_com_acento() {
        let mut x = t("ação");
        inserir(&mut x, 2, "X", 0, Fmt::default());
        assert_eq!(plano(&x), "açXão");
        apagar(&mut x, 1, 3);
        assert_eq!(plano(&x), "aão");
        assert_eq!(tamanho(&x), 3);
    }

    #[test]
    fn inserir_herda_o_estilo_dado_e_junta() {
        let mut x = t("um **dois** três");
        inserir(&mut x, 6, "X", NEGRITO, Fmt::default());
        assert_eq!(x.iter().find(|y| y.estilo == NEGRITO).unwrap().texto, "doiXs");
        assert_eq!(marca_em(&x, 5).0, NEGRITO);
        assert_eq!(marca_em(&x, 0).0, 0);
    }

    #[test]
    fn dividir_e_juntar_preservam_a_formatacao() {
        let x = t("a **bc** d");
        let (e, d) = dividir(&x, 3);
        assert_eq!(plano(&e), "a b");
        assert_eq!(plano(&d), "c d");
        assert_eq!(juntar(e, d), x);
    }

    #[test]
    fn aplicar_estilo_so_no_intervalo() {
        let mut x = t("abcdef");
        aplicar_estilo(&mut x, 2, 4, ITALICO, true);
        assert_eq!(x.len(), 3);
        assert!(todo_com_estilo(&x, 2, 4, ITALICO));
        assert!(!todo_com_estilo(&x, 1, 4, ITALICO));
        aplicar_estilo(&mut x, 0, 6, ITALICO, false);
        assert_eq!(x.len(), 1);
    }

    #[test]
    fn palavras() {
        let c: Vec<char> = "um dois, três".chars().collect();
        assert_eq!(palavra_em(&c, 5), (3, 7));
        assert_eq!(palavra_em(&c, 7), (7, 8));
        assert_eq!(proxima_palavra(&c, 0), 3);
        assert_eq!(proxima_palavra(&c, 4), 9);
        assert_eq!(palavra_anterior(&c, 9), 3);
        assert_eq!(palavra_anterior(&c, 13), 9);
    }

    #[test]
    fn objeto_e_um_caractere_que_nao_se_funde() {
        let mut x = t("ab");
        inserir_objeto(&mut x, 1, Marca::Campo { campo: "autor".into() });
        assert_eq!(tamanho(&x), 3);
        assert!(matches!(objeto_em(&x, 1), Some(Marca::Campo { .. })));
        inserir(&mut x, 2, "Z\u{FFFC}", 0, Fmt::default());
        assert_eq!(x.len(), 3, "o texto digitado depois do objeto não entra nele");
        assert_eq!(crate::inline::texto_puro(&x), "aZb");
        apagar(&mut x, 1, 2);
        assert_eq!(plano(&x), "aZb");
        assert_eq!(x.len(), 1);
    }

    #[test]
    fn link_continua_so_por_dentro() {
        let mut x = t("[abc](u)");
        inserir(&mut x, 1, "X", 0, Fmt::default());
        assert_eq!(x.len(), 1);
        assert_eq!(x[0].link.as_deref(), Some("u"));
        inserir(&mut x, 4, "Y", 0, Fmt::default());
        assert_eq!(x.last().unwrap().link, None, "no fim do link, o texto novo não é link");
    }
}
