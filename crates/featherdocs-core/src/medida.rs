//! A largura dos caracteres, em Rust, para o motor quebrar linha sem perguntar
//! ao desenho. As tabelas são as métricas (AFM) da Helvetica e da Helvetica
//! Negrito, que Arial e Liberation Sans reproduzem — é a fonte que a interface
//! usa para pintar, então a quebra do Rust bate com o que aparece.
//!
//! Aproximação assumida: acento tem a largura da letra-base, itálico tem a do
//! romano, e fora do Latin-1 vale 556. Trocar isto por `cosmic-text` (shaping
//! de verdade, ver `docs/ARQUITETURA.md`) muda só este arquivo.

use crate::modelo::{CODIGO, EXIBIDA, ITALICO, MATEMATICA, NEGRITO, SERIFA};

/// De `' '` (32) a `'~'` (126), em milésimos de em.
const REGULAR: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // espaço ! " # $ % & ' ( ) * + , - . /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0-9 : ; < = > ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ A-O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P-Z [ \ ] ^ _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` a-o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p-z { | } ~
];

const NEGRITADO: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, //
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, //
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, //
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, //
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, //
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, //
];

/// Liberation Serif (as métricas do Times): uma tabela por face, porque no Times o itálico e o negrito têm larguras próprias.
const SERIFA_REGULAR: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444,
    921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722,
    556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500,
    333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500,
    500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

const SERIFA_NEGRITO: [u16; 95] = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    930, 722, 667, 722, 722, 667, 611, 778, 778, 389, 500, 778, 667, 944, 722, 778,
    611, 778, 722, 556, 667, 722, 722, 1000, 722, 722, 667, 333, 278, 333, 581, 500,
    333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556, 278, 833, 556, 500,
    556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
];

const SERIFA_ITALICO: [u16; 95] = [
    250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500,
    920, 611, 611, 667, 722, 611, 611, 722, 722, 333, 444, 667, 556, 833, 667, 722,
    611, 722, 611, 500, 556, 722, 611, 833, 611, 556, 556, 389, 278, 389, 422, 500,
    333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444, 278, 722, 500, 500,
    500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541,
];

const SERIFA_NEGRITO_ITALICO: [u16; 95] = [
    250, 389, 555, 500, 500, 833, 778, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    832, 667, 667, 667, 722, 667, 667, 722, 778, 389, 500, 667, 611, 889, 722, 722,
    611, 722, 667, 556, 611, 722, 667, 889, 667, 611, 611, 333, 278, 333, 570, 500,
    333, 500, 500, 444, 500, 444, 333, 500, 556, 278, 278, 500, 278, 778, 556, 500,
    500, 500, 389, 389, 278, 556, 444, 667, 500, 444, 389, 348, 220, 348, 570,
];

/// Largura de cada caractere de código (monoespaçada), em milésimos de em.
const MONO: u16 = 600;

/// A letra sem acento que dá a largura de um caractere acentuado.
pub(crate) fn base_do_acento(c: char) -> char {
    match c {
        'À'..='Å' => 'A',
        'Ç' => 'C',
        'È'..='Ë' => 'E',
        'Ì'..='Ï' => 'I',
        'Ñ' => 'N',
        'Ò'..='Ö' | 'Ø' => 'O',
        'Ù'..='Ü' => 'U',
        'Ý' => 'Y',
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' | 'ø' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// A largura de `c` em milésimos de em, no `estilo` (negrito, código e a fonte serifada mudam a largura).
pub fn largura_do_caractere(c: char, estilo: u8) -> u16 {
    if estilo & CODIGO != 0 {
        return MONO;
    }
    let c = base_do_acento(c);
    let tabela = if estilo & SERIFA != 0 {
        match (estilo & NEGRITO != 0, estilo & ITALICO != 0) {
            (false, false) => &SERIFA_REGULAR,
            (true, false) => &SERIFA_NEGRITO,
            (false, true) => &SERIFA_ITALICO,
            (true, true) => &SERIFA_NEGRITO_ITALICO,
        }
    } else if estilo & NEGRITO != 0 {
        &NEGRITADO
    } else {
        &REGULAR
    };
    match c {
        ' '..='~' => tabela[c as usize - 32],
        '\u{a0}' => 278,
        '–' => 556,
        '—' => 1000,
        '•' => 350,
        '’' | '‘' => 222,
        '“' | '”' => 333,
        '…' => 1000,
        _ => 556,
    }
}

/// A largura do texto, em pontos, no `tamanho` (pt). Com `MATEMATICA`, o texto é o LaTeX da
/// fórmula e a largura é a da fórmula composta (`feather-math`).
pub fn largura(texto: &str, estilo: u8, tamanho: f32) -> f32 {
    if estilo & MATEMATICA != 0 {
        return feather_math::largura(texto, modo_da_formula(estilo), tamanho);
    }
    let mil: i32 = texto.chars().map(|c| i32::from(largura_do_caractere(c, estilo))).sum();
    let pares: i32 = texto.chars().zip(texto.chars().skip(1)).map(|(a, b)| i32::from(kerning(a, b, estilo))).sum();
    (mil + pares) as f32 * tamanho / 1000.0
}

/// Quanto o texto anda depois de `c` (a largura dele e o kerning com o `seguinte`), em pontos.
pub fn avanco(c: char, seguinte: Option<char>, estilo: u8, tamanho: f32) -> f32 {
    let par = seguinte.map_or(0, |b| kerning(c, b, estilo));
    (f32::from(largura_do_caractere(c, estilo)) + f32::from(par)) * tamanho / 1000.0
}

/// O ajuste entre os caracteres vizinhos `a` e `b` (o kerning), em milésimos de em. A tela e o PDF
/// somam o mesmo ao avançar de uma letra para a outra, para cursor, seleção e quebra coincidirem.
pub fn kerning(a: char, b: char, estilo: u8) -> i16 {
    if estilo & MATEMATICA != 0 {
        return 0;
    }
    crate::kerning::par(a, b, estilo)
}

/// Fórmula em linha ou em destaque, pelo bit `EXIBIDA`.
pub fn modo_da_formula(estilo: u8) -> feather_math::Modo {
    if estilo & EXIBIDA != 0 {
        feather_math::Modo::Exibida
    } else {
        feather_math::Modo::Linha
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn kerning_aproxima_os_pares_e_o_avanco_soma_igual_a_largura() {
        assert!(largura("AV", 0, 10.0) < largura("A", 0, 10.0) + largura("V", 0, 10.0));
        assert!(kerning('T', 'á', SERIFA) < 0, "a letra acentuada usa o par da letra-base");
        assert_eq!(kerning('A', 'V', CODIGO), 0, "o código é monoespaçado");
        assert_eq!(kerning(' ', 'A', 0), 0, "o espaço não entra: a fronteira das palavras não muda");
        for (texto, estilo) in [("Tá AVô, “Yes” y.", 0), ("WAVE Tomo", SERIFA | NEGRITO)] {
            let cs: Vec<char> = texto.chars().collect();
            let soma: f32 = cs.iter().enumerate().map(|(k, &c)| avanco(c, cs.get(k + 1).copied(), estilo, 12.0)).sum();
            assert!((soma - largura(texto, estilo, 12.0)).abs() < 0.01, "{texto}");
        }
    }

    #[test]
    fn helvetica_conhecida() {
        assert_eq!(largura_do_caractere('A', 0), 667);
        assert_eq!(largura_do_caractere('á', 0), 556);
        assert_eq!(largura_do_caractere('i', NEGRITO), 278);
        assert_eq!(largura_do_caractere('x', CODIGO), 600);
    }

    #[test]
    fn largura_em_pontos() {
        // "Hello" em Helvetica 10 pt: (722+556+222+222+556)/100
        assert!((largura("Hello", 0, 10.0) - 22.78).abs() < 0.01);
    }
}
